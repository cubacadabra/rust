use crate::engine::Engine;
use crate::game_package::GamePackageDefinition;
use crate::math::horizontal_distance;
use crate::types::{AgentPhase, BuildBlock};
use crate::world::{LaunchPad, Portal, RuntimeWorld, block_bounds, slot_offset};

impl Engine {
    pub(crate) fn set_launch_pad(
        &mut self,
        index: usize,
        x: f32,
        z: f32,
        radius: f32,
        countdown: f32,
    ) {
        if index >= self.launch_pads.len() {
            self.launch_pads.resize(index + 1, LaunchPad::default());
        }
        if let Some(pad) = self.launch_pads.get_mut(index) {
            *pad = LaunchPad::new(x, z, radius, countdown);
        }
    }

    pub(crate) fn set_launch_pad_count(&mut self, count: usize) {
        self.launch_pads.resize(count.min(64), LaunchPad::default());
    }

    pub(crate) fn set_obstacle(&mut self, index: usize, position: [f32; 3], size: [f32; 3]) {
        if index >= self.obstacles.len() {
            self.obstacles
                .resize(index + 1, block_bounds([0.0; 3], [0.0; 3]));
        }
        if let Some(obstacle) = self.obstacles.get_mut(index) {
            *obstacle = block_bounds(position, size);
        }
    }

    pub(crate) fn set_obstacle_count(&mut self, count: usize) {
        self.obstacles
            .resize(count.min(256), block_bounds([0.0; 3], [0.0; 3]));
    }

    pub(crate) fn set_world_count(&mut self, count: usize) {
        self.worlds.resize(count.min(64), RuntimeWorld::default());
    }

    pub(crate) fn set_world_spawn(&mut self, world: usize, spawn: [f32; 3]) {
        if let Some(world) = self.worlds.get_mut(world) {
            world.spawn = spawn;
        }
    }

    pub(crate) fn set_world_launch_pad_count(&mut self, world: usize, count: usize) {
        if let Some(world) = self.worlds.get_mut(world) {
            let count = count.min(64);
            world.launch_pads.resize(count, LaunchPad::default());
            world.launch_destinations.resize(count, None);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set_world_launch_pad(
        &mut self,
        world: usize,
        index: usize,
        x: f32,
        z: f32,
        radius: f32,
        countdown: f32,
    ) {
        let Some(world) = self.worlds.get_mut(world) else {
            return;
        };
        if index >= world.launch_pads.len() {
            world.launch_pads.resize(index + 1, LaunchPad::default());
            world.launch_destinations.resize(index + 1, None);
        }
        world.launch_pads[index] = LaunchPad::new(x, z, radius, countdown);
    }

    pub(crate) fn set_world_launch_destination(
        &mut self,
        world: usize,
        pad: usize,
        destination: i32,
    ) {
        let world_count = self.worlds.len();
        let Some(world) = self.worlds.get_mut(world) else {
            return;
        };
        if pad >= world.launch_destinations.len() {
            world.launch_destinations.resize(pad + 1, None);
        }
        world.launch_destinations[pad] = usize::try_from(destination)
            .ok()
            .filter(|index| *index < world_count);
    }

    pub(crate) fn set_world_obstacle_count(&mut self, world: usize, count: usize) {
        if let Some(world) = self.worlds.get_mut(world) {
            world
                .obstacles
                .resize(count.min(256), block_bounds([0.0; 3], [0.0; 3]));
        }
    }

    pub(crate) fn set_world_obstacle(
        &mut self,
        world: usize,
        index: usize,
        position: [f32; 3],
        size: [f32; 3],
    ) {
        let Some(world) = self.worlds.get_mut(world) else {
            return;
        };
        if index >= world.obstacles.len() {
            world
                .obstacles
                .resize(index + 1, block_bounds([0.0; 3], [0.0; 3]));
        }
        world.obstacles[index] = block_bounds(position, size);
    }

    pub(crate) fn start_world(&mut self, index: usize) -> bool {
        let Some(world) = self.worlds.get(index).cloned() else {
            return false;
        };
        self.active_world = index;
        self.launch_pads = world.launch_pads;
        self.obstacles = world.obstacles;
        self.base_obstacles = self.obstacles.clone();
        self.build_blocks.clear();
        self.player.position = world.spawn;
        self.player.velocity = [0.0; 3];
        self.player.grounded = true;
        self.agents.clear();
        self.next_spawn_at = self.elapsed + 3.0;
        self.write_snapshot();
        true
    }

    pub(crate) fn set_build_block_count(&mut self, count: usize) {
        self.build_blocks
            .resize(count.min(256), BuildBlock::default());
        self.rebuild_build_obstacles();
    }

    pub(crate) fn set_build_block(
        &mut self,
        index: usize,
        position: [f32; 3],
        size: [f32; 3],
        color: u32,
        rotation: u8,
    ) {
        if let Some(block) = self.build_blocks.get_mut(index) {
            *block = BuildBlock {
                position,
                size,
                color,
                rotation: rotation % 4,
            };
            self.rebuild_build_obstacles();
        }
    }

    pub(crate) fn build_blocks(&self) -> &[BuildBlock] {
        &self.build_blocks
    }

    fn rebuild_build_obstacles(&mut self) {
        self.obstacles = self.base_obstacles.clone();
        self.obstacles.extend(self.build_blocks.iter().map(|block| {
            let size = if block.rotation % 2 == 0 {
                block.size
            } else {
                [block.size[2], block.size[1], block.size[0]]
            };
            block_bounds(block.position, size)
        }));
    }

    pub(crate) fn enter_session(&mut self, launch_pad_index: usize, spawn: [f32; 3]) -> usize {
        let Some(launch_pad) = self.launch_pads.get(launch_pad_index).copied() else {
            return 0;
        };

        let mut selected_agents = self
            .agents
            .iter()
            .copied()
            .filter(|agent| {
                agent.meeting_index == launch_pad_index
                    && agent.phase == AgentPhase::Assembled
                    && horizontal_distance(
                        agent.position[0],
                        agent.position[2],
                        launch_pad.x,
                        launch_pad.z,
                    ) <= launch_pad.radius
            })
            .collect::<Vec<_>>();

        for (index, agent) in selected_agents.iter_mut().enumerate() {
            let offset = slot_offset(index);
            agent.position = [spawn[0] + offset.0, spawn[1], spawn[2] + offset.1];
            agent.meeting_index = 0;
            agent.meeting_target.x = spawn[0];
            agent.meeting_target.z = spawn[2];
            agent.target = agent.meeting_target;
        }
        self.agents = selected_agents;
        self.launch_pads.clear();
        self.next_spawn_at = f32::MAX;
        self.player.position = spawn;
        self.player.velocity = [0.0; 3];
        self.player.grounded = true;
        self.player.moving = false;
        self.player.sprinting = false;
        self.write_snapshot();
        self.agents.len() + 1
    }

    pub(crate) fn load_package_buffer(&mut self) -> bool {
        let source = String::from_utf8_lossy(&self.package_buffer);
        let Ok(package) = GamePackageDefinition::parse(&source) else {
            return false;
        };
        let entries = package.world_entries();
        let world_indices = entries
            .iter()
            .enumerate()
            .map(|(index, (id, _))| (id.as_str(), index))
            .collect::<std::collections::BTreeMap<_, _>>();
        let worlds = entries
            .iter()
            .map(|(id, definition)| {
                let launch_pads = definition
                    .launch_pads
                    .iter()
                    .map(|pad| {
                        let mut launch_pad =
                            LaunchPad::new(pad.x(), pad.z(), pad.radius, pad.countdown);
                        launch_pad.enabled = pad.enabled;
                        launch_pad
                    })
                    .collect::<Vec<_>>();
                let launch_destinations = definition
                    .launch_pads
                    .iter()
                    .map(|pad| {
                        pad.destination_world
                            .as_deref()
                            .or_else(|| {
                                (id == "lobby")
                                    .then_some(package.launch.destination_world.as_deref())
                                    .flatten()
                            })
                            .and_then(|destination| world_indices.get(destination).copied())
                    })
                    .collect::<Vec<_>>();
                let obstacles = definition
                    .blocks
                    .iter()
                    .map(|block| block_bounds(block.position(), block.size()))
                    .collect::<Vec<_>>();
                let portals = definition
                    .portals
                    .iter()
                    .filter_map(|portal| {
                        let destination = world_indices
                            .get(portal.destination_world.as_str())
                            .copied()?;
                        let fallback_spawn = entries.get(destination)?.1.world.spawn();
                        Some(Portal {
                            x: portal.x(),
                            z: portal.z(),
                            radius: portal.radius.max(0.2),
                            destination,
                            destination_spawn: portal.destination_spawn(fallback_spawn),
                            destination_yaw: portal.destination_yaw,
                        })
                    })
                    .collect::<Vec<_>>();
                RuntimeWorld {
                    spawn: definition.world.spawn(),
                    launch_pads,
                    launch_destinations,
                    obstacles,
                    portals,
                }
            })
            .collect::<Vec<_>>();
        let Some(start_world) = world_indices.get(package.start_world.as_str()).copied() else {
            return false;
        };

        self.worlds = worlds;
        self.world_ids = entries.into_iter().map(|(id, _)| id).collect();
        self.package = Some(package);
        self.authoritative_launch = self
            .package
            .as_ref()
            .is_some_and(|package| package.launch.authoritative);
        self.package_generation = self.package_generation.wrapping_add(1).max(1);
        self.start_world(start_world)
    }
}
