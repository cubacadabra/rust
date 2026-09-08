//! Package, username, script, and content-buffer integration.

use crate::character::definition::{CharacterAppearance, CharacterColors};
use crate::engine::Engine;
use crate::game_package::{AvatarDefinition, GamePackageDefinition};
use crate::scripting::GameScript;

impl Engine {
    pub fn load_package_source(&mut self, source: &str) -> bool {
        self.package_buffer.clear();
        self.package_buffer.extend_from_slice(source.as_bytes());
        self.load_package_buffer()
    }

    pub fn load_script_source(&mut self, source: &str) -> bool {
        self.script_buffer.clear();
        self.script_buffer.extend_from_slice(source.as_bytes());
        self.load_script_buffer()
    }

    pub(crate) fn apply_package_default_appearance(&mut self, package: &GamePackageDefinition) {
        if self.player_appearance_persistent {
            return;
        }
        let Some(definition) = package.avatars.player.as_ref() else {
            return;
        };
        let legacy = legacy_colors(definition, CharacterAppearance::default().colors);
        let fallback = CharacterAppearance::default();
        let appearance = definition
            .character
            .as_ref()
            .map(|character| {
                super::remote::resolve_character_definition(character, legacy, &fallback)
            })
            .map(|resolution| resolution.appearance)
            .unwrap_or_else(|| CharacterAppearance {
                colors: legacy,
                ..fallback
            });
        self.player_appearance = appearance;
        self.appearance_generation = self.appearance_generation.wrapping_add(1).max(1);
    }
    pub(crate) fn prepare_script_buffer(&mut self, length: usize) -> *mut u8 {
        self.script_buffer.resize(length, 0);
        self.script_buffer.as_mut_ptr()
    }

    pub(crate) fn prepare_package_buffer(&mut self, length: usize) -> *mut u8 {
        self.package_buffer.resize(length, 0);
        self.package_buffer.as_mut_ptr()
    }

    pub(crate) fn prepare_username_buffer(&mut self, length: usize) -> *mut u8 {
        self.username_buffer.resize(length, 0);
        self.username_buffer.as_mut_ptr()
    }

    pub(crate) fn load_username_buffer(&mut self) -> bool {
        let Ok(source) = std::str::from_utf8(&self.username_buffer) else {
            return false;
        };
        let username = source
            .trim()
            .chars()
            .filter(|character| {
                character.is_ascii_alphanumeric() || matches!(character, ' ' | '_' | '-')
            })
            .take(24)
            .collect::<String>();
        if username.len() < 2 {
            return false;
        }
        self.username = username;
        true
    }

    pub(crate) fn load_script_buffer(&mut self) -> bool {
        let source = String::from_utf8_lossy(&self.script_buffer).into_owned();
        match GameScript::load(&source, std::rc::Rc::clone(&self.ui)) {
            Ok(script) => {
                let direct_world_id = (script.lobby_enabled_override() == Some(false))
                    .then(|| {
                        self.package
                            .as_ref()
                            .and_then(|package| package.direct_world_id().map(str::to_owned))
                    })
                    .flatten();
                self.effects = crate::effects::EffectRuntime::default();
                self.script = Some(script);
                self.script_error_buffer.clear();
                if let Some(world_id) = direct_world_id {
                    if let Some(index) = self.world_ids.iter().position(|id| id == &world_id) {
                        self.start_world(index);
                    }
                } else {
                    self.queue_player_spawn();
                }
                true
            }
            Err(error) => {
                self.script = None;
                self.script_error_buffer = error.into_bytes();
                false
            }
        }
    }

    pub(crate) fn script_error_buffer(&self) -> &[u8] {
        &self.script_error_buffer
    }

    pub(crate) fn script_loaded(&self) -> bool {
        self.script.is_some()
    }

    pub(crate) fn receive_network_message_json(&mut self, source: &str) -> bool {
        self.script
            .as_ref()
            .is_some_and(|script| script.enqueue_network_message(source))
    }

    pub(crate) fn prepare_network_receive_buffer(&mut self, length: usize) -> *mut u8 {
        if length > crate::engine::identity::MAX_NETWORK_MESSAGE_BYTES {
            self.network_receive_buffer.clear();
            return std::ptr::null_mut();
        }
        self.network_receive_buffer.resize(length, 0);
        self.network_receive_buffer.as_mut_ptr()
    }

    pub(crate) fn load_network_receive_buffer(&mut self) -> bool {
        let Some(source) = crate::engine::identity::bounded_utf8(
            &self.network_receive_buffer,
            crate::engine::identity::MAX_NETWORK_MESSAGE_BYTES,
        )
        .map(str::to_owned) else {
            return false;
        };
        self.receive_network_message_json(&source)
    }

    pub(crate) fn poll_network_message(&mut self) -> bool {
        let Some(message) = self
            .script
            .as_ref()
            .and_then(GameScript::take_network_message)
        else {
            self.network_message_buffer.clear();
            return false;
        };
        self.network_message_buffer = message.into_bytes();
        true
    }

    pub(crate) fn network_message(&self) -> &[u8] {
        &self.network_message_buffer
    }

    pub(crate) fn poll_audio_message(&mut self) -> bool {
        let Some(message) = self
            .script
            .as_ref()
            .and_then(GameScript::take_audio_message)
        else {
            self.audio_message_buffer.clear();
            return false;
        };
        self.audio_message_buffer = message.into_bytes();
        true
    }

    pub(crate) fn audio_message(&self) -> &[u8] {
        &self.audio_message_buffer
    }
}

fn legacy_colors(definition: &AvatarDefinition, mut fallback: CharacterColors) -> CharacterColors {
    if let Some(value) = definition.skin.as_deref().and_then(parse_color) {
        fallback.skin = value;
    }
    if let Some(value) = definition.shirt.as_deref().and_then(parse_color) {
        fallback.primary = value;
    }
    if let Some(value) = definition.pants.as_deref().and_then(parse_color) {
        fallback.secondary = value;
    }
    if let Some(value) = definition.shoes.as_deref().and_then(parse_color) {
        fallback.sole = value;
    }
    fallback
}

fn parse_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim().trim_start_matches('#');
    (value.len() == 6)
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
        .map(|rgb| {
            [
                ((rgb >> 16) & 0xff) as f32 / 255.0,
                ((rgb >> 8) & 0xff) as f32 / 255.0,
                (rgb & 0xff) as f32 / 255.0,
                1.0,
            ]
        })
}
