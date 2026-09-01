use std::f32::consts::PI;

use crate::engine::{
    Engine, GRAVITY, JUMP_VELOCITY, MAX_AGENTS, RUN_SPEED, WALK_SPEED, WORLD_LIMIT,
};
use crate::math::{Vec2, horizontal_distance};
use crate::types::AgentPhase;
use crate::world::{entry_point, slot_offset};

impl Engine {
    pub(crate) fn spawn_agents(&mut self) {
        while self.agents.len() < MAX_AGENTS && self.elapsed >= self.next_spawn_at {
            let index = self.agents.len();
            let entry = entry_point(index);
            let meeting_index = entry.2;
            let slot_index = index / 3;
            let gate = self.gates[meeting_index];
            let offset = slot_offset(slot_index);
            let position = [
                entry.0 + self.random.between(-0.7, 0.7),
                0.0,
                entry.1 + self.random.between(-0.4, 0.4),
            ];
            self.agents.push(crate::types::Agent {
                position,
                target: Vec2 {
                    x: entry.3,
                    z: entry.4,
                },
                meeting_target: Vec2 {
                    x: gate.x + offset.0,
                    z: gate.z + offset.1,
                },
                meeting_index,
                phase: AgentPhase::Entering,
                spawned_at: self.elapsed,
                next_decision_at: self.elapsed + self.random.between(1.3, 2.8),
                gather_at: self.elapsed + self.random.between(7.5, 10.5),
                next_jump_at: self.elapsed + self.random.between(1.4, 3.5),
                speed: self.random.between(0.82, 1.08),
                walk_cycle: self.random.between(0.0, PI * 2.0),
                vertical_velocity: 0.0,
                grounded: true,
            });
            self.next_spawn_at += 3.0;
        }
    }

    pub(crate) fn update_agents(&mut self, delta: f32) {
        for index in 0..self.agents.len() {
            self.update_agent(index, delta);
        }
    }

    fn update_agent(&mut self, index: usize, delta: f32) {
        let mut agent = self.agents[index];
        if agent.phase == AgentPhase::Entering
            && horizontal_distance(
                agent.position[0],
                agent.position[2],
                agent.target.x,
                agent.target.z,
            ) < 1.2
        {
            agent.phase = AgentPhase::Roaming;
            agent.target = self.roam_target(agent.position);
            agent.next_decision_at = self.elapsed + self.random.between(1.2, 2.7);
        }

        if agent.phase == AgentPhase::Roaming {
            if self.elapsed >= agent.gather_at {
                agent.phase = AgentPhase::Assembling;
                agent.target = agent.meeting_target;
            } else if self.elapsed >= agent.next_decision_at
                || horizontal_distance(
                    agent.position[0],
                    agent.position[2],
                    agent.target.x,
                    agent.target.z,
                ) < 1.0
            {
                agent.target = self.roam_target(agent.position);
                agent.next_decision_at = self.elapsed + self.random.between(1.1, 2.4);
            }
        }

        if agent.phase == AgentPhase::Assembling
            && horizontal_distance(
                agent.position[0],
                agent.position[2],
                agent.meeting_target.x,
                agent.meeting_target.z,
            ) < 0.65
        {
            agent.phase = AgentPhase::Assembled;
            agent.target = agent.meeting_target;
        }

        if agent.phase != AgentPhase::Assembled {
            let mut direction = Vec2 {
                x: agent.target.x - agent.position[0],
                z: agent.target.z - agent.position[2],
            }
            .normalized();
            self.add_separation(index, &mut direction);
            self.avoid_obstacles(agent.position, &mut direction);
            let is_running = if agent.phase == AgentPhase::Entering {
                (self.elapsed * 1.35 + agent.spawned_at).sin() > -0.25
            } else {
                (self.elapsed * 1.1 + agent.spawned_at * 2.0).sin() > 0.35
            };
            let speed = (if is_running {
                RUN_SPEED
            } else {
                WALK_SPEED * 0.62
            }) * agent.speed;
            agent.position[0] =
                (agent.position[0] + direction.x * speed * delta).clamp(-WORLD_LIMIT, WORLD_LIMIT);
            agent.position[2] =
                (agent.position[2] + direction.z * speed * delta).clamp(-WORLD_LIMIT, WORLD_LIMIT);
            agent.walk_cycle += delta * if is_running { 13.0 } else { 9.0 };
        } else {
            agent.walk_cycle += delta * 2.2;
        }

        if agent.grounded
            && agent.phase != AgentPhase::Assembled
            && self.elapsed >= agent.next_jump_at
        {
            agent.vertical_velocity = JUMP_VELOCITY * self.random.between(0.78, 0.95);
            agent.grounded = false;
            agent.next_jump_at = self.elapsed + self.random.between(3.8, 7.2);
        }
        if !agent.grounded {
            agent.vertical_velocity -= GRAVITY * delta;
            agent.position[1] += agent.vertical_velocity * delta;
            if agent.position[1] <= 0.0 {
                agent.position[1] = 0.0;
                agent.vertical_velocity = 0.0;
                agent.grounded = true;
            }
        }

        self.agents[index] = agent;
    }

    fn roam_target(&mut self, position: [f32; 3]) -> Vec2 {
        let angle = self.random.between(-PI, PI);
        let distance = self.random.between(2.5, 6.5);
        Vec2 {
            x: (position[0] + angle.cos() * distance).clamp(-22.0, 22.0),
            z: (position[2] + angle.sin() * distance).clamp(-1.0, 14.0),
        }
    }

    fn add_separation(&self, index: usize, direction: &mut Vec2) {
        let agent = self.agents[index];
        for (other_index, other) in self.agents.iter().enumerate() {
            if index == other_index {
                continue;
            }
            let offset = Vec2 {
                x: agent.position[0] - other.position[0],
                z: agent.position[2] - other.position[2],
            };
            let distance = offset.length();
            if !(0.001..1.55).contains(&distance) {
                continue;
            }
            let strength = (1.55 - distance) / 1.55;
            direction.x += offset.x / distance * strength * 1.8;
            direction.z += offset.z / distance * strength * 1.8;
        }
        *direction = direction.normalized();
    }

    fn avoid_obstacles(&self, position: [f32; 3], direction: &mut Vec2) {
        for obstacle in &self.obstacles {
            if obstacle.top > 0.8 {
                continue;
            }
            let closest_x = position[0].clamp(obstacle.min_x, obstacle.max_x);
            let closest_z = position[2].clamp(obstacle.min_z, obstacle.max_z);
            let offset = Vec2 {
                x: position[0] - closest_x,
                z: position[2] - closest_z,
            };
            let distance = offset.length();
            if !(0.001..2.1).contains(&distance) {
                continue;
            }
            let strength = (2.1 - distance) / 2.1;
            direction.x += offset.x / distance * strength * 2.5;
            direction.z += offset.z / distance * strength * 2.5;
        }
        *direction = direction.normalized();
    }
}
