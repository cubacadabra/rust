use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CameraDefinition {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) distance: f32,
}

impl CameraDefinition {
    pub(super) fn is_valid(&self) -> bool {
        self.yaw.is_finite()
            && self.yaw.abs() <= std::f32::consts::TAU
            && self.pitch.is_finite()
            && self.pitch.abs() <= crate::engine::MAX_PITCH
            && self.distance.is_finite()
            && (0.0..=crate::engine::MAX_CAMERA_DISTANCE).contains(&self.distance)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PresentationBoundsDefinition {
    pub(crate) minimum: [f32; 3],
    pub(crate) maximum: [f32; 3],
}

impl PresentationBoundsDefinition {
    pub(super) fn is_valid(&self) -> bool {
        self.minimum
            .iter()
            .chain(self.maximum.iter())
            .all(|value| value.is_finite())
            && (0..3).all(|axis| self.minimum[axis] < self.maximum[axis])
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VisualSettingsDefinition {
    /// Legacy material-local grade controls. New packages should use
    /// `colorCorrection`, which applies once to the completed world scene.
    #[serde(default = "default_exposure")]
    pub(crate) exposure: f32,
    #[serde(default = "default_contrast")]
    pub(crate) contrast: f32,
    #[serde(default = "default_saturation")]
    pub(crate) saturation: f32,
    #[serde(default = "default_fog_start")]
    pub(crate) fog_start: f32,
    #[serde(default = "default_fog_end")]
    pub(crate) fog_end: f32,
    #[serde(default = "default_sun_direction")]
    pub(crate) sun_direction: [f32; 3],
    #[serde(default)]
    pub(crate) color_correction: Option<ColorCorrectionDefinition>,
    #[serde(default)]
    pub(crate) daylight: Option<DaylightDefinition>,
    #[serde(default)]
    pub(crate) sun_rays: Option<SunRaysDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ColorCorrectionDefinition {
    #[serde(default)]
    pub(crate) brightness: f32,
    #[serde(default)]
    pub(crate) contrast: f32,
    #[serde(default)]
    pub(crate) saturation: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DaylightDefinition {
    #[serde(default = "default_time_of_day")]
    pub(crate) time_of_day: f32,
    #[serde(default)]
    pub(crate) geographic_latitude: f32,
    #[serde(default = "default_daylight_brightness")]
    pub(crate) brightness: f32,
    #[serde(default = "default_outdoor_ambient")]
    pub(crate) outdoor_ambient: [f32; 3],
    #[serde(default)]
    pub(crate) shadow_softness: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SunRaysDefinition {
    #[serde(default)]
    pub(crate) intensity: f32,
    #[serde(default)]
    pub(crate) spread: f32,
}

impl Default for VisualSettingsDefinition {
    fn default() -> Self {
        Self {
            exposure: default_exposure(),
            contrast: default_contrast(),
            saturation: default_saturation(),
            fog_start: default_fog_start(),
            fog_end: default_fog_end(),
            sun_direction: default_sun_direction(),
            color_correction: None,
            daylight: None,
            sun_rays: None,
        }
    }
}

impl VisualSettingsDefinition {
    pub(super) fn is_valid(&self) -> bool {
        self.exposure.is_finite()
            && self.contrast.is_finite()
            && self.saturation.is_finite()
            && self.fog_start.is_finite()
            && self.fog_end.is_finite()
            && self.sun_direction.iter().all(|value| value.is_finite())
            && self
                .color_correction
                .is_none_or(|correction| correction.is_valid())
            && self.daylight.is_none_or(|daylight| daylight.is_valid())
            && self.sun_rays.is_none_or(|sun_rays| sun_rays.is_valid())
    }
}

impl ColorCorrectionDefinition {
    pub(super) fn is_valid(&self) -> bool {
        [self.brightness, self.contrast, self.saturation]
            .iter()
            .all(|value| value.is_finite() && (-1.0..=1.0).contains(value))
    }
}

impl DaylightDefinition {
    pub(super) fn is_valid(&self) -> bool {
        self.time_of_day.is_finite()
            && (0.0..24.0).contains(&self.time_of_day)
            && self.geographic_latitude.is_finite()
            && (-89.0..=89.0).contains(&self.geographic_latitude)
            && self.brightness.is_finite()
            && (0.0..=4.0).contains(&self.brightness)
            && self
                .outdoor_ambient
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            && self.shadow_softness.is_finite()
            && (0.0..=1.0).contains(&self.shadow_softness)
    }
}

impl SunRaysDefinition {
    pub(super) fn is_valid(&self) -> bool {
        self.intensity.is_finite()
            && self.spread.is_finite()
            && (0.0..=1.0).contains(&self.intensity)
            && (0.0..=1.0).contains(&self.spread)
    }
}

fn default_exposure() -> f32 {
    1.0
}

fn default_contrast() -> f32 {
    1.0
}

fn default_saturation() -> f32 {
    1.0
}

fn default_fog_start() -> f32 {
    52.0
}

fn default_fog_end() -> f32 {
    115.0
}

fn default_sun_direction() -> [f32; 3] {
    [-0.45, -0.82, 0.32]
}

fn default_time_of_day() -> f32 {
    12.0
}

fn default_daylight_brightness() -> f32 {
    2.0
}

fn default_outdoor_ambient() -> [f32; 3] {
    [0.5, 0.5, 0.5]
}
