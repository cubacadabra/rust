#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Vec2 {
    pub(crate) x: f32,
    pub(crate) z: f32,
}

impl Vec2 {
    pub(crate) fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    fn length_squared(self) -> f32 {
        self.x * self.x + self.z * self.z
    }

    pub(crate) fn normalized(self) -> Self {
        let length = self.length();
        if length <= f32::EPSILON {
            Self::default()
        } else {
            Self {
                x: self.x / length,
                z: self.z / length,
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Random(u32);

impl Random {
    pub(crate) fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u32 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.0 = value;
        value
    }

    fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }

    pub(crate) fn between(&mut self, min: f32, max: f32) -> f32 {
        min + self.unit() * (max - min)
    }
}

pub(crate) fn damp(current: f32, target: f32, smoothing: f32, delta: f32) -> f32 {
    current + (target - current) * (1.0 - (-smoothing * delta).exp())
}

pub(crate) fn horizontal_distance(x: f32, z: f32, other_x: f32, other_z: f32) -> f32 {
    ((x - other_x).powi(2) + (z - other_z).powi(2)).sqrt()
}

pub(crate) fn bool_as_float(value: bool) -> f32 {
    if value { 1.0 } else { 0.0 }
}
