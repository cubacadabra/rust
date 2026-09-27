use serde::Deserialize;

use super::Engine;
use crate::engine::{BODY_HEIGHT, PLAYER_RADIUS};
use crate::world::{PendingPush, overlaps_obstacle};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorldBlockState {
    content_hash: String,
    block_index: usize,
    x: f32,
    z: f32,
    sequence: u64,
    request_id: Option<u64>,
    sender_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorldBlockRejection {
    content_hash: String,
    block_index: usize,
    request_id: u64,
    x: f32,
    z: f32,
    sequence: u64,
}

impl Engine {
    pub(crate) fn refresh_pushable_content_hash(&mut self) {
        self.pushable_content_hash = format!(
            "{:016x}",
            super::snapshot::content_fingerprint(&self.package_buffer, &self.script_buffer)
        );
    }

    pub fn set_network_player_id(&mut self, id: Option<String>) {
        self.network_player_id = id;
    }

    pub fn reset_pushable_network(&mut self, online: bool) {
        self.reset_pushable_network_state(online);
    }

    pub fn receive_world_block_state_json(&mut self, source: &str) -> bool {
        self.receive_world_block_state(source)
    }

    pub fn receive_world_block_rejection_json(&mut self, source: &str) -> bool {
        let Ok(rejection) = serde_json::from_str::<WorldBlockRejection>(source) else {
            return false;
        };
        if rejection.content_hash != self.pushable_content_hash
            || !rejection.x.is_finite()
            || !rejection.z.is_finite()
            || rejection.x.abs() > 1000.0
            || rejection.z.abs() > 1000.0
        {
            return false;
        }
        let Some(index) = self
            .pushable_blocks
            .iter()
            .position(|block| block.block_index == rejection.block_index)
        else {
            return false;
        };
        let block = &mut self.pushable_blocks[index];
        if !block
            .pending
            .iter()
            .any(|pending| pending.request_id == rejection.request_id)
        {
            return true;
        }
        block
            .pending
            .retain(|pending| pending.request_id != rejection.request_id);
        if rejection.sequence > block.sequence {
            block.sequence = rejection.sequence;
            block.authoritative_offset = [rejection.x, rejection.z];
        }
        let mut offset = block.authoritative_offset;
        for pending in &block.pending {
            offset[0] += pending.delta[0];
            offset[1] += pending.delta[1];
        }
        self.set_pushable_offset(index, offset);
        true
    }

    pub(crate) fn record_pushable_motion(&mut self, index: usize, axis: usize, travel: f32) {
        if !self.pushable_online {
            return;
        }
        self.next_push_request_id = self.next_push_request_id.saturating_add(1);
        let request_id = self.next_push_request_id;
        let mut delta = [0.0; 2];
        delta[axis / 2] = travel;
        let block = &mut self.pushable_blocks[index];
        block.pending.push(PendingPush { request_id, delta });
        self.pushable_outbox.push_back(
            serde_json::json!({
                "type": "world_block_move",
                "blockIndex": block.block_index,
                "dx": delta[0],
                "dz": delta[1],
                "requestId": request_id,
                "contentHash": self.pushable_content_hash,
            })
            .to_string(),
        );
    }

    pub(crate) fn receive_world_block_state(&mut self, source: &str) -> bool {
        let Ok(state) = serde_json::from_str::<WorldBlockState>(source) else {
            return false;
        };
        if !state.x.is_finite()
            || !state.z.is_finite()
            || state.x.abs() > 1000.0
            || state.z.abs() > 1000.0
            || state.sequence == 0
            || state.content_hash != self.pushable_content_hash
        {
            return false;
        }
        let Some(index) = self
            .pushable_blocks
            .iter()
            .position(|block| block.block_index == state.block_index)
        else {
            return false;
        };
        let block = &mut self.pushable_blocks[index];
        if state.sequence <= block.sequence {
            return true;
        }
        block.sequence = state.sequence;
        block.authoritative_offset = [state.x, state.z];
        if state.sender_id.as_deref() == self.network_player_id.as_deref()
            && let Some(request_id) = state.request_id
        {
            block
                .pending
                .retain(|pending| pending.request_id > request_id);
        }
        let mut offset = block.authoritative_offset;
        for pending in &block.pending {
            offset[0] += pending.delta[0];
            offset[1] += pending.delta[1];
        }
        self.set_pushable_offset(index, offset);
        true
    }

    pub(crate) fn set_pushable_offset(&mut self, index: usize, offset: [f32; 2]) {
        let Some(source_world) = self.worlds.get(self.active_world) else {
            return;
        };
        let block = &self.pushable_blocks[index];
        let old_offset = block.offset;
        if old_offset == offset {
            return;
        }
        let attached_build = self.attached_build_blocks(&block.id);
        let obstacle_indices = std::iter::once(block.obstacle_index)
            .chain(block.attached_obstacles.iter().copied())
            .collect::<Vec<_>>();
        for obstacle_index in obstacle_indices {
            let Some(original) = source_world.obstacles.get(obstacle_index).copied() else {
                continue;
            };
            let mut moved = original;
            moved.min_x += offset[0] + block.script_shift[0];
            moved.max_x += offset[0] + block.script_shift[0];
            moved.min_z += offset[1] + block.script_shift[2];
            moved.max_z += offset[1] + block.script_shift[2];
            moved.bottom += block.script_shift[1];
            moved.top += block.script_shift[1];
            self.base_obstacles[obstacle_index] = moved;
            self.obstacles[obstacle_index] = moved;
        }
        self.pushable_blocks[index].offset = offset;
        if !attached_build.is_empty() {
            let delta = [offset[0] - old_offset[0], offset[1] - old_offset[1]];
            for (block_index, _) in attached_build {
                self.build_blocks[block_index].position[0] += delta[0];
                self.build_blocks[block_index].position[2] += delta[1];
            }
            self.rebuild_build_obstacles();
        }
        // A remote push can reach the player before its socket packet does.
        // Keep the local capsule outside the newly occupied face.
        let obstacle = self.obstacles[self.pushable_blocks[index].obstacle_index];
        if self.player.position[1] < obstacle.top - 0.05
            && self.player.position[1] + BODY_HEIGHT > obstacle.bottom + 0.05
            && overlaps_obstacle(self.player.position, &obstacle, PLAYER_RADIUS)
        {
            let displacement = [offset[0] - old_offset[0], offset[1] - old_offset[1]];
            let axis = if displacement[0].abs() >= displacement[1].abs() {
                0
            } else {
                2
            };
            let mut candidate = self.player.position;
            candidate[axis] += displacement[axis / 2];
            if self.player_can_occupy(candidate) {
                self.player.position = candidate;
            }
        }
    }

    fn reset_pushable_network_state(&mut self, online: bool) {
        self.pushable_online = online;
        self.network_player_id = None;
        self.pushable_outbox.clear();
        for block in &mut self.pushable_blocks {
            block.pending.clear();
        }
        if online {
            for index in 0..self.pushable_blocks.len() {
                self.set_pushable_offset(index, [0.0; 2]);
                self.pushable_blocks[index].authoritative_offset = [0.0; 2];
                self.pushable_blocks[index].sequence = 0;
            }
        }
    }
}
