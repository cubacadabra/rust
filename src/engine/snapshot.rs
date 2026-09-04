use crate::engine::{Engine, SNAPSHOT_STRIDE};
use crate::math::bool_as_float;
use crate::types::AgentPhase;

impl Engine {
    pub(super) fn write_snapshot(&mut self) {
        self.snapshot.fill(0.0);
        self.snapshot[0..SNAPSHOT_STRIDE].copy_from_slice(&[
            self.player.position[0],
            self.player.position[1],
            self.player.position[2],
            self.view_yaw,
            self.player.walk_cycle,
            bool_as_float(self.player.grounded),
            bool_as_float(self.player.moving),
            bool_as_float(self.player.sprinting),
        ]);
        let local_agent_count = self.local_agent_count();
        for (index, agent) in self.agents.iter().take(local_agent_count).enumerate() {
            let offset = (index + 1) * SNAPSHOT_STRIDE;
            let yaw =
                (agent.target.x - agent.position[0]).atan2(agent.target.z - agent.position[2]);
            self.snapshot[offset..offset + SNAPSHOT_STRIDE].copy_from_slice(&[
                agent.position[0],
                agent.position[1],
                agent.position[2],
                yaw,
                agent.walk_cycle,
                agent.phase.code(),
                agent.meeting_index as f32,
                bool_as_float(agent.phase == AgentPhase::Assembled),
            ]);
        }
        for (index, player) in self
            .remote_players
            .iter()
            .take(self.remote_player_count())
            .enumerate()
        {
            let offset = (local_agent_count + index + 1) * SNAPSHOT_STRIDE;
            self.snapshot[offset..offset + SNAPSHOT_STRIDE].copy_from_slice(&[
                player.position[0],
                player.position[1],
                player.position[2],
                player.yaw,
                player.walk_cycle,
                1.0,
                -1.0,
                0.0,
            ]);
        }
    }
}
