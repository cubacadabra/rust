use crate::character::definition::{
    AppearanceInput, CharacterAppearance, CharacterColors, resolve_appearance,
};
use crate::engine::identity::{
    self, STATUS_APPLIED, STATUS_DUPLICATE, STATUS_FALLBACK, STATUS_INVALID, STATUS_STALE,
};
use crate::engine::{ENABLE_LOCAL_NPCS, Engine, MAX_AGENTS};
use crate::game_package::{AvatarDefinition, CharacterDefinition, GamePackageDefinition};
use crate::scripting::GameScript;
use crate::types::{
    CharacterEntityKey, CharacterEntityKind, CharacterMotionSample, CharacterMotionSource,
    CharacterEmote, CharacterMotionEvent, CharacterSupport, Input, LaunchPadPhase, Player,
    RemotePlayer,
};
use crate::ui::{UiPointerPhase, UiViewport};

impl Engine {
    pub(crate) fn set_input(&mut self, input: Input) {
        self.input = input;
    }

    pub(crate) fn set_ui_viewport(&mut self, viewport: UiViewport) {
        self.ui.borrow_mut().set_viewport(viewport);
    }

    pub(crate) fn set_authenticated(&mut self, authenticated: bool) {
        self.ui.borrow_mut().set_authenticated(authenticated);
    }

    pub(crate) fn ui_node_count(&self) -> usize {
        self.ui.borrow().document_node_count()
    }

    pub(crate) fn ui_hit_test(&mut self, x: f32, y: f32) -> bool {
        self.ui.borrow_mut().is_interactive_at(x, y)
    }

    pub(crate) fn ui_external_link_hit_test(&mut self, x: f32, y: f32) -> bool {
        self.ui.borrow_mut().is_external_link_at(x, y)
    }

    pub(crate) fn ui_shared_modal_visible(&self) -> bool {
        self.ui.borrow().shared_modal_visible()
    }

    pub(crate) fn set_ui_document(&mut self, source: &str) -> bool {
        self.ui.borrow_mut().set_document_json(source).is_ok()
    }

    pub(crate) fn prepare_ui_document_buffer(&mut self, length: usize) -> *mut u8 {
        self.ui_document_buffer.resize(length, 0);
        self.ui_document_buffer.as_mut_ptr()
    }

    pub(crate) fn load_ui_document_buffer(&mut self) -> bool {
        let source = String::from_utf8_lossy(&self.ui_document_buffer).into_owned();
        self.set_ui_document(&source)
    }

    pub(crate) fn ui_pointer(
        &mut self,
        pointer_id: u64,
        phase: UiPointerPhase,
        x: f32,
        y: f32,
    ) -> bool {
        self.ui.borrow_mut().pointer(pointer_id, phase, x, y)
    }

    pub fn step(&mut self, delta: f32) {
        let delta = delta.clamp(0.0, 0.05);
        self.elapsed += delta;
        self.motion_sequence = self.motion_sequence.saturating_add(1);
        self.player_motion_event = CharacterMotionEvent::None;
        self.ui.borrow_mut().advance(delta);
        self.tick_script(delta);
        self.apply_camera_input();
        self.smooth_camera(delta);
        self.update_player(delta);
        self.update_portals();
        if ENABLE_LOCAL_NPCS {
            self.spawn_agents();
            self.update_agents(delta);
        }
        for player in &mut self.remote_players {
            // Emotes are edge-triggered presentation inputs. Keep them alive
            // until the next simulation tick so a host can apply a packet and
            // sync immediately, then clear them before the next sample.
            player.emote = CharacterEmote::None;
            if player.moving {
                let speed = if player.sprinting { 13.0 } else { 9.0 };
                player.walk_cycle += delta * speed;
            } else {
                player.walk_cycle = 0.0;
            }
        }
        self.update_launch_pads();
        self.write_snapshot();
    }

    pub fn snapshot(&self) -> &[f32] {
        &self.snapshot
    }

    pub fn set_reduced_effects(&mut self, reduced: bool) {
        self.reduced_effects = reduced;
    }

    pub(crate) fn reduced_effects(&self) -> bool {
        self.reduced_effects
    }

    pub(crate) fn trigger_local_wave(&mut self) {
        self.player_emote = CharacterEmote::Wave;
        self.player_emote_sequence = self.player_emote_sequence.wrapping_add(1).max(1);
    }

    pub(crate) fn agent_count(&self) -> usize {
        self.local_agent_count() + self.remote_player_count()
    }

    pub(crate) fn local_agent_count(&self) -> usize {
        self.agents
            .len()
            .min(MAX_AGENTS.saturating_sub(self.remote_player_count()))
    }

    pub(crate) fn remote_player_count(&self) -> usize {
        self.remote_players.len().min(MAX_AGENTS)
    }

    pub(crate) fn character_motion_samples(
        &self,
    ) -> impl Iterator<Item = CharacterMotionSample> + '_ {
        let local = CharacterMotionSample {
            key: CharacterEntityKey {
                kind: CharacterEntityKind::LocalPlayer,
                slot: 0,
                generation: 0,
                identity: 0,
            },
            sequence: self.motion_sequence,
            time: self.elapsed,
            position: self.player.position,
            facing_yaw: player_facing_yaw(self.player, self.view_yaw),
            look_yaw: self.view_yaw,
            planar_velocity: Some([self.player.velocity[0], self.player.velocity[2]]),
            vertical_velocity: Some(self.player.velocity[1]),
            support: player_support(self.player),
            stride_phase: self.player.walk_cycle,
            moving: self.player.moving,
            sprinting: self.player.sprinting,
            source: CharacterMotionSource::Simulation,
            event: self.player_motion_event,
            emote: self.player_emote,
            emote_sequence: self.player_emote_sequence,
            appearance_revision: self.player_appearance.revision,
        };
        let sequence = self.motion_sequence;
        let time = self.elapsed;
        std::iter::once(local)
            .chain(
                self.agents
                    .iter()
                    .take(self.local_agent_count())
                    .enumerate()
                    .map(move |(slot, agent)| {
                        let facing_yaw = (agent.target.x - agent.position[0])
                            .atan2(agent.target.z - agent.position[2]);
                        CharacterMotionSample {
                            key: CharacterEntityKey {
                                kind: CharacterEntityKind::LocalNpc,
                                slot,
                                generation: 0,
                                identity: 0,
                            },
                            sequence,
                            time,
                            position: agent.position,
                            facing_yaw,
                            look_yaw: facing_yaw,
                            planar_velocity: None,
                            vertical_velocity: Some(agent.vertical_velocity),
                            support: if agent.grounded {
                                CharacterSupport::Grounded {
                                    height: agent.position[1],
                                }
                            } else {
                                CharacterSupport::Airborne
                            },
                            stride_phase: agent.walk_cycle,
                            moving: agent.phase != crate::types::AgentPhase::Assembled,
                            sprinting: false,
                            source: CharacterMotionSource::Simulation,
                            event: CharacterMotionEvent::None,
                            emote: CharacterEmote::None,
                            emote_sequence: 0,
                            appearance_revision: 0,
                        }
                    }),
            )
            .chain(
                self.remote_players
                    .iter()
                    .filter(move |_| self.remote_world_matches_active())
                    .take(self.remote_player_count())
                    .enumerate()
                    .map(move |(slot, player)| CharacterMotionSample {
                        key: CharacterEntityKey {
                            kind: CharacterEntityKind::RemotePlayer,
                            slot: stable_remote_slot(player, slot),
                            generation: player.generation,
                            identity: remote_identity(player, slot),
                        },
                        sequence,
                        time,
                        position: player.position,
                        facing_yaw: player.yaw,
                        look_yaw: player.look_yaw,
                        planar_velocity: player.planar_velocity,
                        vertical_velocity: player.vertical_velocity,
                        support: player.support,
                        stride_phase: player.walk_cycle,
                        moving: player.moving,
                        sprinting: player.sprinting,
                        source: if player.stable_id.is_empty() {
                            CharacterMotionSource::LegacyRemote
                        } else {
                            CharacterMotionSource::VersionedRemote
                        },
                        event: CharacterMotionEvent::None,
                        emote: player.emote,
                        emote_sequence: player.emote_sequence,
                        appearance_revision: player.appearance.revision,
                    }),
            )
    }

    pub(crate) fn set_remote_player_count(&mut self, count: usize) {
        let count = count.min(MAX_AGENTS);
        // Calling the legacy roster API is also an explicit compatibility
        // boundary. Do not let an old host inherit a newer packet sequence.
        let switching_from_versioned = self.remote_packet_sequence != 0
            || self.remote_players.iter().any(|player| !player.stable_id.is_empty());
        self.remote_packet_sequence = 0;
        if self.remote_players.len() != count || switching_from_versioned {
            // The legacy setter carries no identity. Treat a count boundary
            // as a conservative slot replacement signal for presentation.
            self.remote_generation = self.remote_generation.wrapping_add(1).max(1);
            if switching_from_versioned {
                self.remote_players.clear();
            }
        }
        self.remote_world_id = None;
        self.remote_players
            .resize(count, RemotePlayer::default());
        for player in &mut self.remote_players {
            player.generation = self.remote_generation;
            player.motion_sequence = 0;
            player.emote = CharacterEmote::None;
        }
        self.write_snapshot();
    }

    pub(crate) fn reset_remote_session(&mut self) {
        self.remote_players.clear();
        self.remote_packet_sequence = 0;
        self.remote_world_id = None;
        // Keep the bounded appearance cache: a reconnect may legitimately
        // omit unchanged appearance data, while the new generation still
        // resets motion/presentation state when it is materialized.
        self.remote_update_status = STATUS_APPLIED;
        self.write_snapshot();
    }

    pub(crate) fn set_remote_player(
        &mut self,
        index: usize,
        position: [f32; 3],
        yaw: f32,
        moving: bool,
        sprinting: bool,
    ) {
        if let Some(player) = self.remote_players.get_mut(index) {
            player.position = position;
            player.yaw = yaw;
            player.look_yaw = yaw;
            player.planar_velocity = None;
            player.vertical_velocity = None;
            player.support = CharacterSupport::Unknown;
            player.moving = moving;
            player.sprinting = sprinting;
        }
    }

    pub(crate) fn prepare_appearance_buffer(&mut self, length: usize) -> *mut u8 {
        if length > identity::MAX_APPEARANCE_BYTES {
            self.appearance_buffer.clear();
            self.appearance_status = STATUS_INVALID;
            return std::ptr::null_mut();
        }
        self.appearance_buffer.resize(length, 0);
        self.appearance_buffer.as_mut_ptr()
    }

    pub(crate) fn load_appearance_buffer(&mut self) -> bool {
        let Some(source) = identity::bounded_utf8(
            &self.appearance_buffer,
            identity::MAX_APPEARANCE_BYTES,
        ).map(str::to_owned) else {
            self.appearance_status = STATUS_INVALID;
            return false;
        };
        self.apply_local_appearance_json(&source)
    }

    pub(crate) fn set_local_appearance_json(&mut self, source: &str) -> u8 {
        if source.len() > identity::MAX_APPEARANCE_BYTES || !source.is_ascii() {
            self.appearance_status = STATUS_INVALID;
            return self.appearance_status;
        }
        self.apply_local_appearance_json(source);
        self.appearance_status
    }

    fn apply_local_appearance_json(&mut self, source: &str) -> bool {
        let Ok(definition) = serde_json::from_str::<CharacterDefinition>(source) else {
            self.appearance_status = STATUS_INVALID;
            return false;
        };
        if !definition.bounded() {
            self.appearance_status = STATUS_INVALID;
            return false;
        }
        let resolution = resolve_character_definition(
            &definition,
            self.player_appearance.colors,
            &self.player_appearance,
        );
        let revision = resolution.appearance.revision;
        if revision < self.player_appearance.revision
            || (self.player_appearance_persistent
                && revision == self.player_appearance.revision
                && resolution.appearance != self.player_appearance)
        {
            self.appearance_status = STATUS_STALE;
            return false;
        }
        let fallback = !resolution.issues.is_empty();
        if resolution.appearance != self.player_appearance {
            self.player_appearance = resolution.appearance;
            self.player_appearance_persistent = true;
            self.appearance_generation = self.appearance_generation.wrapping_add(1).max(1);
        }
        self.appearance_status = if fallback {
            STATUS_FALLBACK
        } else {
            STATUS_APPLIED
        };
        true
    }

    pub(crate) fn appearance_revision(&self) -> u32 {
        self.player_appearance.revision
    }

    pub(crate) fn appearance_status(&self) -> u8 {
        self.appearance_status
    }

    pub(crate) fn prepare_remote_update_buffer(&mut self, length: usize) -> *mut u8 {
        if length > identity::MAX_REMOTE_UPDATE_BYTES {
            self.remote_update_buffer.clear();
            self.remote_update_status = STATUS_INVALID;
            return std::ptr::null_mut();
        }
        self.remote_update_buffer.resize(length, 0);
        self.remote_update_buffer.as_mut_ptr()
    }

    pub(crate) fn apply_remote_update_buffer(&mut self) -> bool {
        let Some(source) = identity::bounded_utf8(
            &self.remote_update_buffer,
            identity::MAX_REMOTE_UPDATE_BYTES,
        ).map(str::to_owned) else {
            self.remote_update_status = STATUS_INVALID;
            return false;
        };
        self.apply_remote_update_json(&source)
    }

    pub(crate) fn apply_remote_update_json(&mut self, source: &str) -> bool {
        if source.len() > identity::MAX_REMOTE_UPDATE_BYTES {
            self.remote_update_status = STATUS_INVALID;
            return false;
        }
        let Ok(message) = identity::parse_remote_message(source) else {
            self.remote_update_status = STATUS_INVALID;
            return false;
        };
        if message.sequence < self.remote_packet_sequence {
            self.remote_update_status = STATUS_STALE;
            return false;
        }
        if message.sequence == self.remote_packet_sequence && self.remote_packet_sequence != 0 {
            self.remote_update_status = STATUS_DUPLICATE;
            return false;
        }

        let previous = std::mem::take(&mut self.remote_players);
        let mut used_ids = std::collections::BTreeSet::new();
        let mut next_players = Vec::with_capacity(message.players.len());
        let mut had_fallback = false;
        for update in message.players {
            if !used_ids.insert(update.id.clone()) {
                had_fallback = true;
                continue;
            }
            let old = previous
                .iter()
                .find(|player| player.stable_id == update.id);
            let is_new = old.is_none();
            let mut player = old.cloned().unwrap_or_else(|| {
                let appearance = self
                    .remote_identity_cache
                    .get(&update.id)
                    .cloned()
                    .unwrap_or_default();
                RemotePlayer {
                    stable_id: update.id.clone(),
                    identity: identity::stable_identity(&update.id),
                    generation: 0,
                    appearance,
                    ..RemotePlayer::default()
                }
            });
            let generation = if update.generation == 0 {
                if player.generation == 0 {
                    self.next_remote_generation()
                } else {
                    player.generation
                }
            } else {
                update.generation
            };
            if player.generation != 0 && player.generation != generation {
                // A generation change is a new presentation lifetime, but the
                // account's last accepted appearance remains useful during a
                // reconnect when the packet omits content.
                player.motion_sequence = 0;
                player.emote = CharacterEmote::None;
                player.emote_sequence = 0;
            }
            player.stable_id = update.id.clone();
            player.identity = identity::stable_identity(&update.id);
            player.generation = generation;
            if update.appearance.is_none() {
                // Legacy/older clients remain renderable through the bundled
                // resolved appearance, but make the capability downgrade
                // visible to the host through the status byte.
                had_fallback = true;
            }
            self.apply_remote_appearance(
                &mut player,
                update.appearance.as_ref(),
                is_new,
                &mut had_fallback,
            );

            let incoming_motion_sequence = update.motion_sequence.unwrap_or(message.sequence);
            if incoming_motion_sequence >= player.motion_sequence {
                player.position = update.position;
                player.yaw = update.yaw;
                player.look_yaw = update.look_yaw.unwrap_or(update.yaw);
                player.planar_velocity = update.planar_velocity;
                player.vertical_velocity = update.vertical_velocity;
                player.support = remote_support(update.grounded, update.support_height);
                player.walk_cycle = update.stride_phase.unwrap_or(player.walk_cycle);
                player.moving = update.moving;
                player.sprinting = update.sprinting;
                player.motion_sequence = incoming_motion_sequence;
            }
            player.emote = CharacterEmote::None;
            if let Some(emote_sequence) = update.emote_sequence {
                if emote_sequence > player.emote_sequence {
                    player.emote_sequence = emote_sequence;
                    player.emote = match update.emote.as_deref() {
                        Some("wave") => CharacterEmote::Wave,
                        Some(_) => {
                            had_fallback = true;
                            CharacterEmote::None
                        }
                        None => CharacterEmote::None,
                    };
                }
            }
            self.cache_remote_appearance(update.id.clone(), player.appearance.clone());
            next_players.push(player);
        }
        self.remote_players = next_players;
        self.remote_packet_sequence = message.sequence;
        self.remote_world_id = message.world_id;
        self.remote_update_status = if had_fallback {
            STATUS_FALLBACK
        } else {
            STATUS_APPLIED
        };
        self.write_snapshot();
        true
    }

    fn apply_remote_appearance(
        &self,
        player: &mut RemotePlayer,
        definition: Option<&CharacterDefinition>,
        is_new: bool,
        had_fallback: &mut bool,
    ) {
        let Some(definition) = definition else {
            return;
        };
        let resolution = resolve_character_definition(
            definition,
            player.appearance.colors,
            &player.appearance,
        );
        if resolution.appearance.revision < player.appearance.revision {
            *had_fallback = true;
            return;
        }
        if !is_new
            && resolution.appearance.revision == player.appearance.revision
            && resolution.appearance != player.appearance
        {
            *had_fallback = true;
            return;
        }
        *had_fallback |= !resolution.issues.is_empty();
        player.appearance = resolution.appearance;
    }

    fn next_remote_generation(&mut self) -> u32 {
        self.remote_generation = self.remote_generation.wrapping_add(1).max(1);
        self.remote_generation
    }

    fn cache_remote_appearance(&mut self, id: String, appearance: CharacterAppearance) {
        if !self.remote_identity_cache.contains_key(&id)
            && self.remote_identity_cache.len() >= MAX_AGENTS
        {
            if let Some(oldest) = self.remote_identity_cache.keys().next().cloned() {
                self.remote_identity_cache.remove(&oldest);
            }
        }
        self.remote_identity_cache.insert(id, appearance);
    }

    pub(crate) fn remote_update_status(&self) -> u8 {
        self.remote_update_status
    }

    pub(crate) fn remote_update_sequence(&self) -> u64 {
        self.remote_packet_sequence
    }

    pub(crate) fn player_appearance(&self) -> &CharacterAppearance {
        &self.player_appearance
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
            .map(|character| resolve_character_definition(character, legacy, &fallback))
            .map(|resolution| resolution.appearance)
            .unwrap_or_else(|| CharacterAppearance {
                colors: legacy,
                ..fallback
            });
        self.player_appearance = appearance;
        self.appearance_generation = self.appearance_generation.wrapping_add(1).max(1);
    }

    pub(crate) fn remote_appearance(&self, key: CharacterEntityKey) -> Option<&CharacterAppearance> {
        self.remote_players.iter().find(|player| {
            remote_identity(player, 0) == key.identity && player.generation == key.generation
        }).map(|player| &player.appearance)
    }

    fn remote_world_matches_active(&self) -> bool {
        self.remote_world_id.as_deref().is_none_or(|remote_world| {
            self.world_ids
                .get(self.active_world)
                .is_none_or(|active_world| active_world == remote_world)
        })
    }

    pub(crate) fn elapsed(&self) -> f32 {
        self.elapsed
    }

    pub fn meeting_count(&self, index: usize) -> usize {
        self.count_launch_pad_occupants(index)
    }

    pub(crate) fn launch_pad_occupants(&self, index: usize) -> usize {
        self.launch_pads.get(index).map_or(0, |pad| pad.occupants)
    }

    pub(crate) fn launch_pad_count(&self) -> usize {
        self.launch_pads.len()
    }

    pub(crate) fn launch_pad_seconds(&self, index: usize) -> f32 {
        self.launch_pads.get(index).map_or(0.0, |pad| {
            if pad.phase == LaunchPadPhase::Countdown {
                (pad.launch_at - self.elapsed).max(0.0)
            } else {
                0.0
            }
        })
    }

    pub(crate) fn launch_pad_phase(&self, index: usize) -> u8 {
        self.launch_pads
            .get(index)
            .map_or(0, |pad| pad.phase.code())
    }

    pub(crate) fn player_launch_pad(&self) -> i32 {
        self.launch_pads
            .iter()
            .position(|pad| self.player_is_on_pad(pad))
            .map_or(-1, |index| index as i32)
    }

    pub(crate) fn launch_event_id(&self) -> u32 {
        self.launch_event_id
    }

    pub(crate) fn last_launch_pad(&self) -> usize {
        self.last_launch_pad
    }

    pub(crate) fn last_launch_occupants(&self) -> usize {
        self.last_launch_occupants
    }

    pub(crate) fn active_world(&self) -> usize {
        self.active_world
    }

    pub(crate) fn settings_room_state(&self) -> u8 {
        let Some(room) = self
            .package
            .as_ref()
            .and_then(|package| package.settings_room.as_ref())
        else {
            return 0;
        };
        if self.world_ids.get(self.active_world).map(String::as_str) != Some(room.world_id.as_str())
        {
            return 0;
        }

        let distance = (self.player.position[0] - room.username_station_x())
            .hypot(self.player.position[2] - room.username_station_z());
        if distance <= room.interaction_radius.max(0.0) {
            2
        } else {
            1
        }
    }

    pub(crate) fn world_event_id(&self) -> u32 {
        self.world_event_id
    }

    pub(crate) fn last_world_source_pad(&self) -> usize {
        self.last_world_source_pad
    }

    pub(crate) fn last_world_destination(&self) -> usize {
        self.last_world_destination
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
                self.script = Some(script);
                self.script_error_buffer.clear();
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
}

fn resolve_character_definition(
    definition: &CharacterDefinition,
    legacy_colors: CharacterColors,
    fallback: &CharacterAppearance,
) -> crate::character::definition::AppearanceResolution {
    let equipment = &definition.equipment;
    let colors = &definition.colors;
    resolve_appearance(AppearanceInput {
        version: definition.version.or(Some(1)),
        body: definition
            .body
            .as_deref()
            .or(Some(fallback.body.stable_id())),
        face: definition
            .face
            .as_deref()
            .or(Some(fallback.face.stable_id())),
        outfit: definition
            .outfit
            .as_deref()
            .or(Some(fallback.outfit.stable_id())),
        equipment,
        colors,
        legacy_colors,
        revision: definition.revision,
    })
}

fn remote_support(grounded: Option<bool>, height: Option<f32>) -> CharacterSupport {
    match grounded {
        Some(true) => CharacterSupport::Grounded {
            height: height.unwrap_or(0.0),
        },
        Some(false) => CharacterSupport::Airborne,
        None => CharacterSupport::Unknown,
    }
}

fn remote_identity(player: &RemotePlayer, slot: usize) -> u64 {
    if player.identity != 0 {
        player.identity
    } else {
        // Old setters have no account key. Keep their existing slot-scoped
        // identity explicit so a later reuse cannot inherit presentation
        // state accidentally.
        0x9e37_79b9_7f4a_7c15_u64 ^ (slot as u64).wrapping_mul(0x1000_0000_01b3)
    }
}

fn stable_remote_slot(player: &RemotePlayer, slot: usize) -> usize {
    remote_identity(player, slot) as usize
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

fn player_facing_yaw(player: Player, fallback: f32) -> f32 {
    let planar_speed = player.velocity[0].hypot(player.velocity[2]);
    if planar_speed > 0.001 {
        (-player.velocity[0]).atan2(-player.velocity[2])
    } else {
        fallback
    }
}

fn player_support(player: Player) -> CharacterSupport {
    if player.grounded {
        CharacterSupport::Grounded {
            height: player.position[1],
        }
    } else {
        CharacterSupport::Airborne
    }
}
