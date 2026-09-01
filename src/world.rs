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
pub(crate) struct Gate {
    pub(crate) x: f32,
    pub(crate) z: f32,
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
