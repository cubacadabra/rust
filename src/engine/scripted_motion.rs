use super::Engine;
use crate::scripting::spatial::BodyObservation;

impl Engine {
    pub(super) fn sync_spatial_observations(&self) {
        let Some(script) = &self.script else {
            return;
        };
        let state = script.state();
        let mut state = state.borrow_mut();
        if state.spatial.watched.is_empty() {
            return;
        }
        state.spatial.player = Some(self.player.position);
        for value in state.spatial.watched.values_mut() {
            *value = None;
        }
        for block in &self.pushable_blocks {
            if let Some(value) = state.spatial.watched.get_mut(&block.id) {
                let bounds = self.base_obstacles[block.obstacle_index];
                *value = Some(BodyObservation {
                    position: [
                        (bounds.min_x + bounds.max_x) * 0.5,
                        (bounds.bottom + bounds.top) * 0.5,
                        (bounds.min_z + bounds.max_z) * 0.5,
                    ],
                    offset: block.offset,
                    shift: block.script_shift,
                });
            }
        }
    }

    pub(super) fn apply_spatial_commands(&mut self) {
        let Some(script) = &self.script else {
            return;
        };
        let (shifts, impulse) = {
            let state = script.state();
            let mut state = state.borrow_mut();
            (
                std::mem::take(&mut state.spatial.shifts),
                std::mem::take(&mut state.spatial.impulse),
            )
        };
        for (id, shift) in shifts {
            if let Some(index) = self.pushable_blocks.iter().position(|body| body.id == id) {
                self.set_scripted_shift(index, shift);
            }
        }
        if impulse != [0.0; 3] && !self.player_dead {
            for (velocity, amount) in self.player.velocity.iter_mut().zip(impulse) {
                *velocity = (*velocity + amount).clamp(-30.0, 30.0);
            }
            if impulse[1] > 0.0 {
                self.player.grounded = false;
            }
        }
    }

    pub(crate) fn set_scripted_shift(&mut self, index: usize, shift: [f32; 3]) {
        let block = &self.pushable_blocks[index];
        let previous = block.script_shift;
        if previous == shift {
            return;
        }
        let bounds = self.obstacles[block.obstacle_index];
        let carried = self.player.grounded
            && (self.player.position[1] - bounds.top).abs() < 0.08
            && self.player.position[0] > bounds.min_x - 0.1
            && self.player.position[0] < bounds.max_x + 0.1
            && self.player.position[2] > bounds.min_z - 0.1
            && self.player.position[2] < bounds.max_z + 0.1;
        let delta = [
            shift[0] - previous[0],
            shift[1] - previous[1],
            shift[2] - previous[2],
        ];
        for obstacle_index in
            std::iter::once(block.obstacle_index).chain(block.attached_obstacles.iter().copied())
        {
            let mut moved = self.base_obstacles[obstacle_index];
            moved.min_x += delta[0];
            moved.max_x += delta[0];
            moved.bottom += delta[1];
            moved.top += delta[1];
            moved.min_z += delta[2];
            moved.max_z += delta[2];
            self.base_obstacles[obstacle_index] = moved;
            self.obstacles[obstacle_index] = moved;
        }
        self.pushable_blocks[index].script_shift = shift;
        // Carry riders on small platform movements. Keep avatars out of a
        // kinematic body which reaches them; never drag them through a wall.
        if carried || !self.player_can_occupy(self.player.position) {
            let candidate = [
                self.player.position[0] + delta[0],
                self.player.position[1] + delta[1],
                self.player.position[2] + delta[2],
            ];
            if self.player_can_occupy(candidate) {
                self.player.position = candidate;
            } else {
                let mut above = self.player.position;
                above[1] = self.obstacles[self.pushable_blocks[index].obstacle_index].top + 0.05;
                if self.player_can_occupy(above) {
                    self.player.position = above;
                }
            }
        }
    }
}
