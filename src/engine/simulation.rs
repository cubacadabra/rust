use crate::engine::Engine;
use crate::math::horizontal_distance;
use crate::types::{AgentPhase, LaunchPadPhase};
use crate::world::{LaunchPad, Portal};

impl Engine {
    pub(super) fn tick_script(&mut self, delta: f32) {
        if let Some(script) = &self.script
            && let Err(error) = script.tick(delta)
        {
            script.state().borrow_mut().last_error = Some(error);
        }
    }

    pub(super) fn update_launch_pads(&mut self) {
        let mut launched = None;
        for index in 0..self.launch_pads.len() {
            let occupants = self.count_launch_pad_occupants(index);
            let local_player_selected = self
                .launch_pads
                .get(index)
                .is_some_and(|pad| self.player_is_on_pad(pad));
            let pad = &mut self.launch_pads[index];
            if !pad.enabled {
                pad.occupants = 0;
                pad.phase = LaunchPadPhase::Idle;
                pad.launch_at = 0.0;
                continue;
            }
            pad.occupants = occupants;

            if self.authoritative_launch {
                continue;
            }

            match pad.phase {
                LaunchPadPhase::Idle if occupants > 0 => {
                    pad.phase = LaunchPadPhase::Countdown;
                    pad.launch_at = self.elapsed + pad.countdown;
                }
                LaunchPadPhase::Countdown if occupants == 0 => {
                    pad.phase = LaunchPadPhase::Idle;
                    pad.launch_at = 0.0;
                }
                LaunchPadPhase::Countdown if self.elapsed >= pad.launch_at => {
                    pad.phase = LaunchPadPhase::Launched;
                    self.launch_event_id = self.launch_event_id.wrapping_add(1);
                    self.last_launch_pad = index;
                    self.last_launch_occupants = occupants;
                    launched = Some((index, occupants, local_player_selected));
                }
                LaunchPadPhase::Launched if occupants == 0 => {
                    pad.phase = LaunchPadPhase::Idle;
                    pad.launch_at = 0.0;
                }
                _ => {}
            }
        }

        if let Some((index, occupants, local_player_selected)) = launched {
            let pad_id = format!("pad-{index}");
            let player_ids = (0..occupants as u32).collect::<Vec<_>>();
            if let Some(script) = &self.script
                && let Err(error) = script.launch(&pad_id, &player_ids)
            {
                script.state().borrow_mut().last_error = Some(error);
            }

            let destination = self
                .worlds
                .get(self.active_world)
                .and_then(|world| world.launch_destinations.get(index))
                .copied()
                .flatten();
            if local_player_selected && let Some(destination) = destination {
                self.enter_registered_world(index, destination);
            }
        }
    }

    pub(super) fn update_portals(&mut self) {
        if self.elapsed < self.portal_cooldown_until {
            return;
        }
        let portal = self
            .worlds
            .get(self.active_world)
            .and_then(|world| {
                world.portals.iter().find(|portal| {
                    horizontal_distance(
                        self.player.position[0],
                        self.player.position[2],
                        portal.x,
                        portal.z,
                    ) <= portal.radius
                })
            })
            .copied();
        if let Some(portal) = portal {
            self.enter_portal(portal);
        }
    }

    fn enter_portal(&mut self, portal: Portal) {
        let Some(world) = self.worlds.get(portal.destination).cloned() else {
            return;
        };
        self.active_world = portal.destination;
        if let Some(world_id) = self.world_ids.get(portal.destination).cloned() {
            self.ui.borrow_mut().set_world_id(&world_id);
        }
        self.launch_pads = world.launch_pads;
        self.obstacles = world.obstacles;
        self.base_obstacles = self.obstacles.clone();
        self.build_blocks.clear();
        self.player.position = portal.destination_spawn;
        self.player.velocity = [0.0; 3];
        self.player.grounded = true;
        self.player.moving = false;
        self.player.sprinting = false;
        self.view_yaw = portal.destination_yaw;
        self.target_yaw = portal.destination_yaw;
        self.view_pitch = -0.095;
        self.target_pitch = -0.095;
        self.agents.clear();
        self.remote_players.clear();
        self.next_spawn_at = f32::MAX;
        self.portal_cooldown_until = self.elapsed + 0.6;
        self.world_event_id = self.world_event_id.wrapping_add(1);
        self.last_world_destination = portal.destination;
    }

    fn enter_registered_world(&mut self, source_pad: usize, destination: usize) {
        let Some(world) = self.worlds.get(destination).cloned() else {
            return;
        };
        self.enter_session(source_pad, world.spawn);
        self.launch_pads = world.launch_pads;
        self.obstacles = world.obstacles;
        self.base_obstacles = self.obstacles.clone();
        self.build_blocks.clear();
        self.active_world = destination;
        if let Some(world_id) = self.world_ids.get(destination).cloned() {
            self.ui.borrow_mut().set_world_id(&world_id);
        }
        self.world_event_id = self.world_event_id.wrapping_add(1);
        self.last_world_source_pad = source_pad;
        self.last_world_destination = destination;
    }

    pub(super) fn count_launch_pad_occupants(&self, index: usize) -> usize {
        let Some(pad) = self.launch_pads.get(index) else {
            return 0;
        };
        let agents = self
            .agents
            .iter()
            .filter(|agent| {
                agent.meeting_index == index
                    && agent.phase == AgentPhase::Assembled
                    && horizontal_distance(agent.position[0], agent.position[2], pad.x, pad.z)
                        <= pad.radius
            })
            .count();
        agents + usize::from(self.player_is_on_pad(pad))
    }

    pub(super) fn player_is_on_pad(&self, pad: &LaunchPad) -> bool {
        pad.enabled
            && self.player.grounded
            && horizontal_distance(
                self.player.position[0],
                self.player.position[2],
                pad.x,
                pad.z,
            ) <= pad.radius
    }
}
