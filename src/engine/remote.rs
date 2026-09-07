//! Remote roster, appearance, and motion synchronization state.

use crate::character::definition::{
    AppearanceInput, CharacterAppearance, CharacterColors, resolve_appearance,
};
use crate::engine::identity::{
    self, STATUS_APPLIED, STATUS_DUPLICATE, STATUS_FALLBACK, STATUS_INVALID, STATUS_STALE,
};
use crate::engine::{Engine, MAX_AGENTS};
use crate::game_package::CharacterDefinition;
use crate::types::{CharacterEmote, CharacterEntityKey, CharacterSupport, RemotePlayer};

impl Engine {
    pub(crate) fn set_remote_player_count(&mut self, count: usize) {
        let count = count.min(MAX_AGENTS);
        // Calling the legacy roster API is also an explicit compatibility
        // boundary. Do not let an old host inherit a newer packet sequence.
        let switching_from_versioned = self.remote_packet_sequence != 0
            || self
                .remote_players
                .iter()
                .any(|player| !player.stable_id.is_empty());
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
        self.remote_players.resize(count, RemotePlayer::default());
        for (index, player) in self.remote_players.iter_mut().enumerate() {
            player.generation = self.remote_generation;
            player.motion_sequence = 0;
            player.emote = CharacterEmote::None;
            if player.stable_id.is_empty() {
                player.appearance.colors.primary =
                    crate::character::definition::vibrant_hoodie_color(index);
            }
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
        let Some(source) =
            identity::bounded_utf8(&self.appearance_buffer, identity::MAX_APPEARANCE_BYTES)
                .map(str::to_owned)
        else {
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
        )
        .map(str::to_owned) else {
            self.remote_update_status = STATUS_INVALID;
            return false;
        };
        self.apply_remote_update_json(&source)
    }

    pub(crate) fn prepare_remote_motion_batch_buffer(&mut self, length: usize) -> *mut u8 {
        if length > identity::MAX_REMOTE_MOTION_BATCH_BYTES {
            self.remote_motion_buffer.clear();
            self.remote_update_status = STATUS_INVALID;
            return std::ptr::null_mut();
        }
        self.remote_motion_buffer.resize(length, 0);
        self.remote_motion_buffer.as_mut_ptr()
    }

    pub(crate) fn apply_remote_motion_batch_buffer(&mut self) -> bool {
        let source = self.remote_motion_buffer.clone();
        self.apply_remote_motion_batch(&source)
    }

    /// Applies the compact hot-path motion ABI. The control-plane roster owns
    /// stable identity and ordering; each record carries the identity hash,
    /// generation and motion sequence so stale packets cannot move a new
    /// presentation lifetime.
    pub(crate) fn apply_remote_motion_batch(&mut self, source: &[u8]) -> bool {
        const HEADER_BYTES: usize = identity::REMOTE_MOTION_BATCH_HEADER_BYTES;
        const RECORD_BYTES: usize = identity::REMOTE_MOTION_RECORD_BYTES;
        if source.len() > identity::MAX_REMOTE_MOTION_BATCH_BYTES || source.len() < HEADER_BYTES {
            self.remote_update_status = STATUS_INVALID;
            return false;
        }

        let version = u32::from_le_bytes(source[0..4].try_into().unwrap());
        let count = u32::from_le_bytes(source[4..8].try_into().unwrap()) as usize;
        let Some(expected_length) =
            HEADER_BYTES.checked_add(count.checked_mul(RECORD_BYTES).unwrap_or(usize::MAX))
        else {
            self.remote_update_status = STATUS_INVALID;
            return false;
        };
        if version != identity::REMOTE_MOTION_PROTOCOL_VERSION
            || count > MAX_AGENTS
            || source.len() != expected_length
        {
            self.remote_update_status = STATUS_INVALID;
            return false;
        }

        let mut records = Vec::with_capacity(count);
        let mut identities = std::collections::BTreeSet::new();
        for index in 0..count {
            let offset = HEADER_BYTES + index * RECORD_BYTES;
            let stable_identity =
                u64::from_le_bytes(source[offset..offset + 8].try_into().unwrap());
            let generation =
                u32::from_le_bytes(source[offset + 8..offset + 12].try_into().unwrap());
            let motion_sequence =
                u64::from_le_bytes(source[offset + 12..offset + 20].try_into().unwrap());
            let position = [
                f32::from_le_bytes(source[offset + 20..offset + 24].try_into().unwrap()),
                f32::from_le_bytes(source[offset + 24..offset + 28].try_into().unwrap()),
                f32::from_le_bytes(source[offset + 28..offset + 32].try_into().unwrap()),
            ];
            let yaw = f32::from_le_bytes(source[offset + 32..offset + 36].try_into().unwrap());
            let flags = u32::from_le_bytes(source[offset + 36..offset + 40].try_into().unwrap());
            if stable_identity == 0
                || flags & !0b11 != 0
                || !identities.insert(stable_identity)
                || !position.iter().all(|value| value.is_finite())
                || !yaw.is_finite()
            {
                self.remote_update_status = STATUS_INVALID;
                return false;
            }
            records.push((
                stable_identity,
                generation,
                motion_sequence,
                position,
                yaw,
                flags,
            ));
        }

        for (stable_identity, generation, motion_sequence, position, yaw, flags) in records {
            let Some(player) = self
                .remote_players
                .iter_mut()
                .find(|player| player.identity == stable_identity)
            else {
                continue;
            };
            if (generation != 0 && player.generation != generation)
                || motion_sequence <= player.motion_sequence
            {
                continue;
            }
            player.position = position;
            player.yaw = yaw;
            player.look_yaw = yaw;
            player.planar_velocity = None;
            player.vertical_velocity = None;
            player.support = CharacterSupport::Unknown;
            player.moving = flags & 0b01 != 0;
            player.sprinting = flags & 0b10 != 0;
            player.motion_sequence = motion_sequence;
        }
        self.remote_update_status = STATUS_APPLIED;
        self.write_snapshot();
        true
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
            let old = previous.iter().find(|player| player.stable_id == update.id);
            // A roster presence is not proof that an appearance revision has
            // been accepted: older clients may join without appearance data.
            // The cache is the durable appearance-revision boundary.
            let is_new = !self.remote_identity_cache.contains_key(&update.id);
            let mut player = old.cloned().unwrap_or_else(|| {
                let appearance = self
                    .remote_identity_cache
                    .get(&update.id)
                    .cloned()
                    .unwrap_or_else(|| {
                        remote_spawn_appearance(
                            identity::stable_identity(&update.id) as usize,
                            &next_players,
                        )
                    });
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
            if let Some(username) = update.username {
                player.display_name = username;
            }
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
            if incoming_motion_sequence > player.motion_sequence {
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
            if update.appearance.is_some() || self.remote_identity_cache.contains_key(&update.id) {
                self.cache_remote_appearance(update.id.clone(), player.appearance.clone());
            }
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
        let resolution =
            resolve_character_definition(definition, player.appearance.colors, &player.appearance);
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

    pub(crate) fn cache_remote_appearance(&mut self, id: String, appearance: CharacterAppearance) {
        if self.remote_identity_cache.contains_key(&id) {
            self.remote_identity_cache_order
                .retain(|cached_id| cached_id != &id);
        } else if self.remote_identity_cache.len() >= MAX_AGENTS {
            if let Some(oldest) = self.remote_identity_cache_order.pop_front() {
                self.remote_identity_cache.remove(&oldest);
            }
        }
        self.remote_identity_cache.insert(id.clone(), appearance);
        self.remote_identity_cache_order.push_back(id);
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

    pub(crate) fn remote_appearance(
        &self,
        key: CharacterEntityKey,
    ) -> Option<&CharacterAppearance> {
        self.remote_players
            .iter()
            .find(|player| {
                remote_identity(player, 0) == key.identity && player.generation == key.generation
            })
            .map(|player| &player.appearance)
    }
}

pub(super) fn resolve_character_definition(
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

fn remote_spawn_appearance(index: usize, occupied: &[RemotePlayer]) -> CharacterAppearance {
    let mut appearance = CharacterAppearance::default();
    appearance.colors.primary = (0..crate::character::definition::VIBRANT_HOODIE_COLORS.len())
        .map(|offset| crate::character::definition::vibrant_hoodie_color(index + offset))
        .find(|candidate| {
            occupied
                .iter()
                .all(|player| player.appearance.colors.primary != *candidate)
        })
        .unwrap_or_else(|| crate::character::definition::vibrant_hoodie_color(index));
    appearance
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

pub(super) fn remote_identity(player: &RemotePlayer, slot: usize) -> u64 {
    if player.identity != 0 {
        player.identity
    } else {
        // Old setters have no account key. Keep their existing slot-scoped
        // identity explicit so a later reuse cannot inherit presentation
        // state accidentally.
        0x9e37_79b9_7f4a_7c15_u64 ^ (slot as u64).wrapping_mul(0x1000_0000_01b3)
    }
}

pub(super) fn stable_remote_slot(player: &RemotePlayer, slot: usize) -> usize {
    remote_identity(player, slot) as usize
}
