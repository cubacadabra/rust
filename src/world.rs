use crate::types::LaunchPadPhase;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Aabb {
    pub(crate) min_x: f32,
    pub(crate) max_x: f32,
    pub(crate) min_z: f32,
    pub(crate) max_z: f32,
    pub(crate) bottom: f32,
    pub(crate) top: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LaunchPad {
    pub(crate) x: f32,
    pub(crate) z: f32,
    pub(crate) radius: f32,
    pub(crate) countdown: f32,
    pub(crate) phase: LaunchPadPhase,
    pub(crate) launch_at: f32,
    pub(crate) occupants: usize,
    pub(crate) enabled: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Portal {
    pub(crate) x: f32,
    pub(crate) z: f32,
    pub(crate) radius: f32,
    pub(crate) destination: usize,
    pub(crate) destination_spawn: [f32; 3],
    pub(crate) destination_yaw: f32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeWorld {
    pub(crate) spawn: [f32; 3],
    pub(crate) launch_pads: Vec<LaunchPad>,
    pub(crate) launch_destinations: Vec<Option<usize>>,
    pub(crate) obstacles: Vec<Aabb>,
    pub(crate) portals: Vec<Portal>,
}

impl LaunchPad {
    pub(crate) fn new(x: f32, z: f32, radius: f32, countdown: f32) -> Self {
        Self {
            x,
            z,
            radius: radius.max(0.1),
            countdown: countdown.max(0.1),
            phase: LaunchPadPhase::Idle,
            launch_at: 0.0,
            occupants: 0,
            enabled: true,
        }
    }
}

impl Default for LaunchPad {
    fn default() -> Self {
        Self::new(0.0, 0.0, 2.7, 8.0)
    }
}

pub(crate) fn block_bounds(position: [f32; 3], size: [f32; 3]) -> Aabb {
    Aabb {
        min_x: position[0] - size[0] / 2.0,
        max_x: position[0] + size[0] / 2.0,
        min_z: position[2] - size[2] / 2.0,
        max_z: position[2] + size[2] / 2.0,
        bottom: position[1] - size[1] / 2.0,
        top: position[1] + size[1] / 2.0,
    }
}

pub(crate) fn overlaps_obstacle(position: [f32; 3], obstacle: &Aabb, radius: f32) -> bool {
    let closest_x = position[0].clamp(obstacle.min_x, obstacle.max_x);
    let closest_z = position[2].clamp(obstacle.min_z, obstacle.max_z);
    let distance_x = position[0] - closest_x;
    let distance_z = position[2] - closest_z;
    distance_x * distance_x + distance_z * distance_z < radius * radius
}

pub(crate) fn entry_point(index: usize) -> (f32, f32, usize, f32, f32) {
    match index % 3 {
        0 => (-17.0, 12.0, 0, -14.0, 4.0),
        1 => (0.0, 16.0, 1, 0.0, 3.0),
        _ => (17.0, 12.0, 2, 14.0, 4.0),
    }
}

pub(crate) fn slot_offset(index: usize) -> (f32, f32) {
    match index % 7 {
        0 => (-1.75, 1.35),
        1 => (0.0, 1.65),
        2 => (1.75, 1.35),
        3 => (-2.05, -0.45),
        4 => (2.05, -0.45),
        5 => (-0.8, -1.7),
        _ => (0.8, -1.7),
    }
}
