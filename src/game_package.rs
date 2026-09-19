use std::collections::BTreeMap;

use cubacadabra_morphs::MorphParameterValue;
use serde::{Deserialize, Deserializer};

use crate::effects::EffectLibraryDefinition;

fn default_start_world() -> String {
    "lobby".to_owned()
}

fn default_lobby_enabled() -> bool {
    true
}

fn default_ground_size() -> f32 {
    120.0
}

fn default_grid_size() -> f32 {
    112.0
}

fn default_grid_divisions() -> usize {
    28
}

fn default_true() -> bool {
    true
}

fn default_radius() -> f32 {
    2.7
}

fn default_countdown() -> f32 {
    8.0
}

fn default_scale() -> f32 {
    1.0
}

fn default_gravity() -> f32 {
    28.0
}

fn default_jump_velocity() -> f32 {
    10.5
}

fn default_ground_collision() -> bool {
    true
}

fn default_void_y() -> f32 {
    -100.0
}

fn default_respawn_delay() -> f32 {
    0.55
}

fn default_climb_speed() -> f32 {
    4.5
}

fn default_max_health() -> f32 {
    100.0
}

fn default_start_health() -> f32 {
    100.0
}

fn default_respawn_mode() -> String {
    "checkpoint".to_owned()
}

pub(crate) const PREVIEW_SDK_VERSION: &str = "0.3.0";
pub(crate) const TERRAIN_SDK_VERSION: &str = "0.4.0";
pub(crate) const CURRENT_SDK_VERSION: &str = "0.5.0";

fn deserialize_sdk_version<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error as _;

    let value = Option::<String>::deserialize(deserializer)?;
    if let Some(version) = value.as_deref()
        && version != PREVIEW_SDK_VERSION
        && version != TERRAIN_SDK_VERSION
        && version != CURRENT_SDK_VERSION
    {
        return Err(D::Error::custom(format!(
            "unsupported sdkVersion {version:?}; runtime supports {PREVIEW_SDK_VERSION}, {TERRAIN_SDK_VERSION}, and {CURRENT_SDK_VERSION}"
        )));
    }
    Ok(value)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GamePackageDefinition {
    #[serde(default, deserialize_with = "deserialize_sdk_version")]
    pub(crate) _sdk_version: Option<String>,
    #[serde(default = "default_start_world")]
    pub(crate) start_world: String,
    /// Lobbies remain the default for existing packages. A package can opt
    /// into direct experience routing with `"lobby": false`.
    #[serde(default = "default_lobby_enabled")]
    pub(crate) lobby: bool,
    #[serde(default)]
    pub(crate) launch: LaunchRouteDefinition,
    #[serde(default)]
    pub(crate) palette: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) terrain: crate::terrain::TerrainDefinition,
    #[serde(default)]
    pub(crate) collision: Option<crate::static_collision::StaticCollisionDefinition>,
    #[serde(default)]
    pub(crate) world: WorldSettingsDefinition,
    #[serde(default)]
    pub(crate) settings_room: Option<SettingsRoomDefinition>,
    #[serde(default)]
    pub(crate) launch_pads: Vec<LaunchPadDefinition>,
    #[serde(default)]
    pub(crate) blocks: Vec<BlockDefinition>,
    #[serde(default)]
    pub(crate) portals: Vec<PortalDefinition>,
    #[serde(default)]
    pub(crate) signs: Vec<SignDefinition>,
    #[serde(default)]
    pub(crate) billboards: Vec<BillboardDefinition>,
    #[serde(default)]
    pub(crate) worlds: BTreeMap<String, WorldDefinition>,
    #[serde(default)]
    pub(crate) avatars: AvatarSetDefinition,
    #[serde(default)]
    pub(crate) effects: EffectLibraryDefinition,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsRoomDefinition {
    pub(crate) world_id: String,
    #[serde(default)]
    pub(crate) username_station_position: Vec<f32>,
    #[serde(default = "default_interaction_radius")]
    pub(crate) interaction_radius: f32,
}

impl SettingsRoomDefinition {
    pub(crate) fn username_station_x(&self) -> f32 {
        self.username_station_position
            .first()
            .copied()
            .unwrap_or(0.0)
    }

    pub(crate) fn username_station_z(&self) -> f32 {
        self.username_station_position
            .get(2)
            .or_else(|| self.username_station_position.get(1))
            .copied()
            .unwrap_or(0.0)
    }
}

fn default_interaction_radius() -> f32 {
    4.0
}

fn default_interaction_kind() -> String {
    "zone".to_owned()
}

impl GamePackageDefinition {
    pub(crate) fn parse(source: &str) -> Result<Self, serde_json::Error> {
        let package: Self = serde_json::from_str(source)?;
        if (!package.terrain.operations.is_empty()
            || package
                .worlds
                .values()
                .any(|world| !world.terrain.operations.is_empty()))
            && package._sdk_version.as_deref() != Some(TERRAIN_SDK_VERSION)
            && package._sdk_version.as_deref() != Some(CURRENT_SDK_VERSION)
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "terrain operations require sdkVersion {TERRAIN_SDK_VERSION} or {CURRENT_SDK_VERSION}"
                ),
            )));
        }
        let has_sdk_05_fields = package.collision.is_some()
            || package
                .worlds
                .values()
                .any(|world| world.collision.is_some())
            || std::iter::once(&package.world)
                .chain(package.worlds.values().map(|world| &world.world))
                .any(|world| {
                    world.camera.is_some()
                        || world.physics.horizontal_bounds.is_some()
                        || world.visual.color_correction.is_some()
                        || world.visual.daylight.is_some()
                        || world.visual.sun_rays.is_some()
                });
        if has_sdk_05_fields && package._sdk_version.as_deref() != Some(CURRENT_SDK_VERSION) {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "collision, world.camera, world.physics.horizontalBounds, and world.visual environment controls require sdkVersion {CURRENT_SDK_VERSION}"
                ),
            )));
        }
        if std::iter::once(&package.world)
            .chain(package.worlds.values().map(|world| &world.world))
            .filter_map(|world| world.presentation_bounds.as_ref())
            .any(|bounds| !bounds.is_valid())
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "world.presentationBounds minimum and maximum must be finite and ordered on every axis",
            )));
        }
        if std::iter::once(&package.world)
            .chain(package.worlds.values().map(|world| &world.world))
            .filter_map(|world| world.physics.horizontal_bounds.as_ref())
            .any(|bounds| !bounds.is_valid())
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "world.physics.horizontalBounds minimum and maximum must be finite and ordered on both axes",
            )));
        }
        if std::iter::once(&package.world)
            .chain(package.worlds.values().map(|world| &world.world))
            .filter_map(|world| world.camera.as_ref())
            .any(|camera| !camera.is_valid())
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "world.camera must contain finite yaw/pitch/distance within runtime limits",
            )));
        }
        if std::iter::once(&package.world)
            .chain(package.worlds.values().map(|world| &world.world))
            .any(|world| !world.visual.is_valid())
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "world.visual environment controls must be finite and within their documented ranges",
            )));
        }
        for collision in std::iter::once(package.collision.as_ref())
            .chain(
                package
                    .worlds
                    .values()
                    .map(|world| world.collision.as_ref()),
            )
            .flatten()
        {
            if let Err(error) = collision.validate() {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    error,
                )));
            }
        }
        Ok(package)
    }

    pub(crate) fn world_entries(&self) -> Vec<(String, WorldDefinition)> {
        let lobby = WorldDefinition {
            palette: self.palette.clone(),
            materials: BTreeMap::new(),
            ground_material: None,
            terrain: self.terrain.clone(),
            collision: self.collision.clone(),
            world: self.world.clone(),
            launch_pads: self.launch_pads.clone(),
            blocks: self.blocks.clone(),
            portals: self.portals.clone(),
            signs: self.signs.clone(),
            billboards: self.billboards.clone(),
            interactions: Vec::new(),
            ladders: Vec::new(),
            checkpoints: Vec::new(),
            hazards: Vec::new(),
            safe_zones: Vec::new(),
            decorations: Vec::new(),
        };
        std::iter::once(("lobby".to_owned(), lobby))
            .chain(
                self.worlds
                    .iter()
                    .map(|(id, world)| (id.clone(), world.clone())),
            )
            .collect()
    }

    pub(crate) fn initial_world_id(&self) -> Option<&str> {
        if self.lobby || self.start_world != "lobby" {
            return Some(self.start_world.as_str());
        }
        self.direct_world_id()
    }

    pub(crate) fn direct_world_id(&self) -> Option<&str> {
        if self.start_world != "lobby" {
            Some(self.start_world.as_str())
        } else {
            self.launch.destination_world.as_deref()
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchRouteDefinition {
    pub(crate) destination_world: Option<String>,
    #[serde(default)]
    pub(crate) authoritative: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorldDefinition {
    #[serde(default)]
    pub(crate) palette: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) materials: BTreeMap<String, MaterialDefinition>,
    #[serde(default)]
    pub(crate) ground_material: Option<String>,
    #[serde(default)]
    pub(crate) terrain: crate::terrain::TerrainDefinition,
    #[serde(default)]
    pub(crate) collision: Option<crate::static_collision::StaticCollisionDefinition>,
    #[serde(default)]
    pub(crate) world: WorldSettingsDefinition,
    #[serde(default)]
    pub(crate) launch_pads: Vec<LaunchPadDefinition>,
    #[serde(default)]
    pub(crate) blocks: Vec<BlockDefinition>,
    #[serde(default)]
    pub(crate) portals: Vec<PortalDefinition>,
    #[serde(default)]
    pub(crate) signs: Vec<SignDefinition>,
    #[serde(default)]
    pub(crate) billboards: Vec<BillboardDefinition>,
    #[serde(default)]
    pub(crate) interactions: Vec<InteractionDefinition>,
    #[serde(default)]
    pub(crate) ladders: Vec<LadderDefinition>,
    #[serde(default)]
    pub(crate) checkpoints: Vec<CheckpointDefinition>,
    #[serde(default)]
    pub(crate) hazards: Vec<HazardDefinition>,
    #[serde(default)]
    pub(crate) safe_zones: Vec<SafeZoneDefinition>,
    #[serde(default)]
    pub(crate) decorations: Vec<DecorationDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaterialDefinition {
    #[serde(default)]
    pub(crate) image: String,
    #[serde(default = "default_material_tile_size")]
    pub(crate) tile_u: f32,
    #[serde(default = "default_material_tile_size")]
    pub(crate) tile_v: f32,
}

fn default_material_tile_size() -> f32 {
    8.0
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HazardDefinition {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default = "default_hazard_kind")]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default)]
    pub(crate) size: Vec<f32>,
    #[serde(default)]
    pub(crate) damage_per_second: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SafeZoneDefinition {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default = "default_safe_zone_radius")]
    pub(crate) radius: f32,
    #[serde(default)]
    pub(crate) heal_per_second: f32,
}

fn default_safe_zone_radius() -> f32 {
    5.0
}

impl SafeZoneDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }
}

fn default_hazard_kind() -> String {
    "damage".to_owned()
}

impl HazardDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }

    pub(crate) fn size(&self) -> [f32; 3] {
        [
            self.size.first().copied().unwrap_or(0.0),
            self.size.get(1).copied().unwrap_or(0.0),
            self.size.get(2).copied().unwrap_or(0.0),
        ]
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InteractionDefinition {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) label: String,
    #[serde(default = "default_interaction_kind")]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default = "default_interaction_radius")]
    pub(crate) radius: f32,
    #[serde(default)]
    pub(crate) color: String,
    #[serde(default)]
    pub(crate) visual: Option<String>,
}

impl InteractionDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorldSettingsDefinition {
    #[serde(default = "default_ground_size")]
    pub(crate) ground_size: f32,
    #[serde(default = "default_grid_size")]
    pub(crate) grid_size: f32,
    #[serde(default = "default_grid_divisions")]
    pub(crate) grid_divisions: usize,
    #[serde(default = "default_true")]
    pub(crate) show_grid: bool,
    #[serde(default)]
    pub(crate) spawn: Vec<f32>,
    #[serde(default)]
    pub(crate) camera: Option<CameraDefinition>,
    #[serde(default = "default_true")]
    pub(crate) show_spawn_pad: bool,
    #[serde(default)]
    pub(crate) clouds: Vec<CloudDefinition>,
    #[serde(default)]
    pub(crate) physics: PhysicsDefinition,
    #[serde(default)]
    pub(crate) health: HealthDefinition,
    #[serde(default)]
    pub(crate) respawn: RespawnDefinition,
    #[serde(default)]
    pub(crate) visual: VisualSettingsDefinition,
    #[serde(default)]
    pub(crate) presentation_bounds: Option<PresentationBoundsDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CameraDefinition {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) distance: f32,
}

impl CameraDefinition {
    fn is_valid(&self) -> bool {
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
    fn is_valid(&self) -> bool {
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
    fn is_valid(&self) -> bool {
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
    fn is_valid(&self) -> bool {
        [self.brightness, self.contrast, self.saturation]
            .iter()
            .all(|value| value.is_finite() && (-1.0..=1.0).contains(value))
    }
}

impl DaylightDefinition {
    fn is_valid(&self) -> bool {
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
    fn is_valid(&self) -> bool {
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

impl Default for WorldSettingsDefinition {
    fn default() -> Self {
        Self {
            ground_size: default_ground_size(),
            grid_size: default_grid_size(),
            grid_divisions: default_grid_divisions(),
            show_grid: true,
            spawn: vec![0.0, 0.0, 0.0],
            camera: None,
            show_spawn_pad: true,
            clouds: Vec::new(),
            physics: PhysicsDefinition::default(),
            health: HealthDefinition::default(),
            respawn: RespawnDefinition::default(),
            visual: VisualSettingsDefinition::default(),
            presentation_bounds: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthDefinition {
    #[serde(default = "default_max_health")]
    pub(crate) max: f32,
    #[serde(default = "default_start_health")]
    pub(crate) start: f32,
}

impl Default for HealthDefinition {
    fn default() -> Self {
        Self {
            max: default_max_health(),
            start: default_start_health(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RespawnDefinition {
    #[serde(default = "default_respawn_mode")]
    pub(crate) mode: String,
    #[serde(default)]
    pub(crate) delay: Option<f32>,
}

impl Default for RespawnDefinition {
    fn default() -> Self {
        Self {
            mode: default_respawn_mode(),
            delay: None,
        }
    }
}

impl WorldSettingsDefinition {
    pub(crate) fn spawn(&self) -> [f32; 3] {
        [
            self.spawn.first().copied().unwrap_or(0.0),
            self.spawn.get(1).copied().unwrap_or(0.0),
            self.spawn.get(2).copied().unwrap_or(0.0),
        ]
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhysicsDefinition {
    #[serde(default = "default_gravity")]
    pub(crate) gravity: f32,
    #[serde(default = "default_jump_velocity")]
    pub(crate) jump_velocity: f32,
    #[serde(default = "default_ground_collision")]
    pub(crate) ground_collision: bool,
    #[serde(default)]
    pub(crate) ground_y: f32,
    #[serde(default = "default_void_y")]
    pub(crate) death_y: f32,
    #[serde(default = "default_respawn_delay")]
    pub(crate) respawn_delay: f32,
    #[serde(default = "default_climb_speed")]
    pub(crate) climb_speed: f32,
    #[serde(default)]
    pub(crate) horizontal_bounds: Option<HorizontalBoundsDefinition>,
}

impl Default for PhysicsDefinition {
    fn default() -> Self {
        Self {
            gravity: default_gravity(),
            jump_velocity: default_jump_velocity(),
            ground_collision: default_ground_collision(),
            ground_y: 0.0,
            death_y: default_void_y(),
            respawn_delay: default_respawn_delay(),
            climb_speed: default_climb_speed(),
            horizontal_bounds: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HorizontalBoundsDefinition {
    pub(crate) minimum: [f32; 2],
    pub(crate) maximum: [f32; 2],
}

impl HorizontalBoundsDefinition {
    fn is_valid(&self) -> bool {
        self.minimum
            .iter()
            .chain(self.maximum.iter())
            .all(|value| value.is_finite())
            && (0..2).all(|axis| self.minimum[axis] < self.maximum[axis])
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LadderDefinition {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default)]
    pub(crate) size: Vec<f32>,
    #[serde(default = "default_ladder_axis")]
    pub(crate) climb_axis: String,
    #[serde(default)]
    pub(crate) color: String,
    #[serde(default)]
    pub(crate) climb_speed: Option<f32>,
}

impl LadderDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }

    pub(crate) fn size(&self) -> [f32; 3] {
        [
            self.size.first().copied().unwrap_or(1.5),
            self.size.get(1).copied().unwrap_or(6.0),
            self.size.get(2).copied().unwrap_or(0.8),
        ]
    }
}

fn default_ladder_axis() -> String {
    "z".to_owned()
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointDefinition {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default = "default_radius")]
    pub(crate) radius: f32,
}

impl CheckpointDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudDefinition {
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default = "default_scale")]
    pub(crate) scale: f32,
}

impl CloudDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchPadDefinition {
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) code: String,
    #[serde(default)]
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default)]
    pub(crate) color: String,
    #[serde(default = "default_radius")]
    pub(crate) radius: f32,
    #[serde(default = "default_countdown")]
    pub(crate) countdown: f32,
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) availability_label: String,
    pub(crate) destination_world: Option<String>,
}

impl LaunchPadDefinition {
    pub(crate) fn x(&self) -> f32 {
        self.position.first().copied().unwrap_or(0.0)
    }

    pub(crate) fn z(&self) -> f32 {
        self.position
            .get(2)
            .or_else(|| self.position.get(1))
            .copied()
            .unwrap_or(0.0)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockDefinition {
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default)]
    pub(crate) size: Vec<f32>,
    #[serde(default)]
    pub(crate) color: String,
    #[serde(default)]
    pub(crate) material: Option<String>,
    #[serde(default = "default_true")]
    pub(crate) outline: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DecorationDefinition {
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default = "default_decoration_scale")]
    pub(crate) scale: f32,
    #[serde(default)]
    pub(crate) scale3: Option<Vec<f32>>,
    #[serde(default)]
    pub(crate) yaw: f32,
    #[serde(default)]
    pub(crate) color: String,
    #[serde(default)]
    pub(crate) variant: usize,
    #[serde(default)]
    pub(crate) asset: Option<String>,
    #[serde(default)]
    pub(crate) material: Option<String>,
}

fn default_decoration_scale() -> f32 {
    1.0
}

impl DecorationDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }

    pub(crate) fn scale3(&self) -> [f32; 3] {
        self.scale3.as_deref().map_or([self.scale; 3], |scale| {
            [
                scale.first().copied().unwrap_or(self.scale),
                scale.get(1).copied().unwrap_or(self.scale),
                scale.get(2).copied().unwrap_or(self.scale),
            ]
        })
    }
}

impl BlockDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }

    pub(crate) fn size(&self) -> [f32; 3] {
        [
            self.size.first().copied().unwrap_or(1.0),
            self.size.get(1).copied().unwrap_or(1.0),
            self.size.get(2).copied().unwrap_or(1.0),
        ]
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PortalDefinition {
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default = "default_portal_radius")]
    pub(crate) radius: f32,
    pub(crate) destination_world: String,
    #[serde(default)]
    pub(crate) destination_spawn: Vec<f32>,
    #[serde(default)]
    pub(crate) destination_yaw: f32,
}

impl PortalDefinition {
    pub(crate) fn x(&self) -> f32 {
        self.position.first().copied().unwrap_or(0.0)
    }

    pub(crate) fn z(&self) -> f32 {
        self.position
            .get(2)
            .or_else(|| self.position.get(1))
            .copied()
            .unwrap_or(0.0)
    }

    pub(crate) fn destination_spawn(&self, fallback: [f32; 3]) -> [f32; 3] {
        if self.destination_spawn.is_empty() {
            return fallback;
        }
        [
            self.destination_spawn
                .first()
                .copied()
                .unwrap_or(fallback[0]),
            self.destination_spawn
                .get(1)
                .copied()
                .unwrap_or(fallback[1]),
            self.destination_spawn
                .get(2)
                .copied()
                .unwrap_or(fallback[2]),
        ]
    }
}

fn default_portal_radius() -> f32 {
    1.25
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SignDefinition {
    #[serde(default)]
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default)]
    pub(crate) yaw: f32,
    #[serde(default = "default_sign_width")]
    pub(crate) max_width: f32,
    #[serde(default)]
    pub(crate) color: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BillboardDefinition {
    pub(crate) image: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default)]
    pub(crate) yaw: f32,
    #[serde(default = "default_billboard_width")]
    pub(crate) width: f32,
    #[serde(default = "default_billboard_height")]
    pub(crate) height: f32,
    #[serde(default = "default_true")]
    pub(crate) framed: bool,
}

impl BillboardDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }
}

fn default_billboard_width() -> f32 {
    7.2
}

fn default_billboard_height() -> f32 {
    4.05
}

impl SignDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        [
            self.position.first().copied().unwrap_or(0.0),
            self.position.get(1).copied().unwrap_or(0.0),
            self.position.get(2).copied().unwrap_or(0.0),
        ]
    }
}

fn default_sign_width() -> f32 {
    5.0
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct AvatarSetDefinition {
    pub(crate) player: Option<AvatarDefinition>,
    #[serde(default)]
    pub(crate) npcs: Vec<AvatarDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct AvatarDefinition {
    pub(crate) skin: Option<String>,
    pub(crate) shirt: Option<String>,
    pub(crate) pants: Option<String>,
    pub(crate) shoes: Option<String>,
    #[serde(default)]
    pub(crate) character: Option<CharacterDefinition>,
}

/// Additive Phase 5 appearance data. The four legacy color strings remain
/// siblings of this object so old packages keep their original shape.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CharacterDefinition {
    #[serde(default)]
    pub(crate) version: Option<u16>,
    #[serde(default)]
    pub(crate) base: Option<String>,
    #[serde(default)]
    pub(crate) parts: Vec<String>,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) face: Option<String>,
    #[serde(default)]
    pub(crate) outfit: Option<String>,
    #[serde(default)]
    pub(crate) equipment: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) colors: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) parameters: BTreeMap<String, MorphParameterValue>,
    #[serde(default)]
    pub(crate) revision: u32,
}

impl CharacterDefinition {
    pub(crate) fn bounded(&self) -> bool {
        self.asset_strings().all(|value| {
            value.len() <= 96
                && value.is_ascii()
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric()
                        || matches!(byte, b':' | b'.' | b'-' | b'_' | b'/' | b'#')
                })
        }) && self.equipment.len() <= 32
            && self.colors.len() <= 16
            && self.estimated_size() <= 4096
    }

    fn asset_strings(&self) -> impl Iterator<Item = &str> {
        self.body
            .iter()
            .chain(self.base.iter())
            .chain(self.parts.iter())
            .chain(self.face.iter())
            .chain(self.outfit.iter())
            .chain(self.equipment.keys())
            .chain(self.equipment.values())
            .chain(self.colors.keys())
            .chain(self.colors.values())
            .chain(self.parameters.keys())
            .map(String::as_str)
    }

    fn estimated_size(&self) -> usize {
        self.asset_strings().map(str::len).sum::<usize>()
            + self.equipment.len() * 8
            + self.colors.len() * 8
            + 32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_external_game_manifest_parses() {
        let Ok(path) = std::env::var("CUBACADABRA_TEST_GAME_MANIFEST") else {
            return;
        };
        let source = std::fs::read_to_string(&path).expect("configured game manifest should exist");
        GamePackageDefinition::parse(&source).expect("configured game manifest should parse");
    }

    #[test]
    fn rejects_an_unsupported_sdk_version() {
        let error = GamePackageDefinition::parse(r#"{"sdkVersion":"0.6.0"}"#)
            .expect_err("unsupported SDK versions must be rejected");
        assert!(error.to_string().contains("unsupported sdkVersion"));
    }

    #[test]
    fn accepts_terrain_sdk_version_and_requires_it_for_terrain_data() {
        let supported = GamePackageDefinition::parse(r#"{"sdkVersion":"0.4.0"}"#)
            .expect("terrain-capable SDK should be accepted");
        assert_eq!(supported._sdk_version.as_deref(), Some(TERRAIN_SDK_VERSION));
        let current = GamePackageDefinition::parse(r#"{"sdkVersion":"0.5.0"}"#)
            .expect("current SDK should be accepted");
        assert_eq!(current._sdk_version.as_deref(), Some(CURRENT_SDK_VERSION));

        let top_level = r##"{
            "sdkVersion":"0.4.0",
            "terrain":{"operations":[{
                "shape":"block","operation":"fill","position":[0,0,0],
                "size":[2,2,2],"material":"builtin:grass"
            }]}
        }"##;
        let package = GamePackageDefinition::parse(top_level)
            .expect("top-level terrain should be supported for the lobby world");
        assert_eq!(package.world_entries()[0].1.terrain.operations.len(), 1);

        let terrain = r##"{
            "sdkVersion":"0.3.0",
            "worlds":{"maze":{"terrain":{"operations":[{
                "shape":"block","operation":"fill","position":[0,0,0],
                "size":[2,2,2],"material":"builtin:grass"
            }]}}}
        }"##;
        let error = GamePackageDefinition::parse(terrain)
            .expect_err("older packages must not silently ignore terrain semantics");
        assert!(
            error
                .to_string()
                .contains("require sdkVersion 0.4.0 or 0.5.0")
        );

        let current_terrain = r##"{
            "sdkVersion":"0.5.0",
            "worlds":{"maze":{"terrain":{"operations":[{
                "shape":"block","operation":"fill","position":[0,0,0],
                "size":[2,2,2],"material":"builtin:grass"
            }]}}}
        }"##;
        GamePackageDefinition::parse(current_terrain)
            .expect("current SDK should retain terrain support");
    }

    #[test]
    fn retains_authored_render_fields() {
        let package = GamePackageDefinition::parse(
            r##"{
                "startWorld":"lobby",
                "palette":{"paper":"#ffffff"},
                "world":{
                    "groundSize":120,
                    "gridSize":84,
                    "gridDivisions":21,
                    "spawn":[1,2,3],
                    "showSpawnPad":false,
                    "presentationBounds":{"minimum":[-4,-2,-3],"maximum":[5,7,6]},
                    "clouds":[{"position":[4,5,6],"scale":1.5}]
                },
                "launchPads":[{
                    "code":"GATE 01",
                    "label":"SUN COURT",
                    "position":[-10,0,-3],
                    "color":"#ed725b"
                }],
                "blocks":[{
                    "position":[0,1,2],
                    "size":[3,4,5],
                    "color":"paper",
                    "outline":false
                }]
            }"##,
        )
        .expect("package should parse");
        let worlds = package.world_entries();
        let lobby = &worlds[0].1;
        assert_eq!(lobby.world.grid_size, 84.0);
        assert_eq!(lobby.world.grid_divisions, 21);
        assert!(!lobby.world.show_spawn_pad);
        let bounds = lobby.world.presentation_bounds.unwrap();
        assert_eq!(bounds.minimum, [-4.0, -2.0, -3.0]);
        assert_eq!(bounds.maximum, [5.0, 7.0, 6.0]);
        assert_eq!(lobby.world.clouds[0].position(), [4.0, 5.0, 6.0]);
        assert_eq!(lobby.launch_pads[0].label, "SUN COURT");
        assert!(!lobby.blocks[0].outline);
    }

    #[test]
    fn retains_source_style_environment_controls() {
        let package = GamePackageDefinition::parse(
            r#"{
                "sdkVersion":"0.5.0",
                "world":{"visual":{
                    "colorCorrection":{"brightness":0.12,"contrast":0.2,"saturation":0.6},
                    "daylight":{"timeOfDay":6.5,"geographicLatitude":45,"brightness":2,"outdoorAmbient":[0.5,0.5,0.5],"shadowSoftness":0.5},
                    "sunRays":{"intensity":0.058,"spread":0.463}
                }}
            }"#,
        )
        .expect("environment controls should parse in the current SDK");
        let visual = &package.world_entries()[0].1.world.visual;
        assert_eq!(visual.color_correction.unwrap().saturation, 0.6);
        assert_eq!(visual.daylight.unwrap().time_of_day, 6.5);
        assert_eq!(visual.sun_rays.unwrap().spread, 0.463);
    }

    #[test]
    fn rejects_invalid_presentation_bounds() {
        let error = GamePackageDefinition::parse(
            r#"{"world":{"presentationBounds":{"minimum":[0,0,0],"maximum":[0,2,3]}}}"#,
        )
        .expect_err("zero-width presentation bounds must not be accepted");
        assert!(error.to_string().contains("presentationBounds"));
    }

    #[test]
    fn parses_and_validates_authored_world_camera() {
        let package = GamePackageDefinition::parse(
            r#"{"sdkVersion":"0.5.0","world":{"camera":{"yaw":1.25,"pitch":0.4,"distance":18}}}"#,
        )
        .expect("authored camera should parse");
        assert_eq!(package.world.camera.unwrap().distance, 18.0);

        for camera in [
            r#"{"yaw":null,"pitch":0,"distance":8}"#,
            r#"{"yaw":0,"pitch":2,"distance":8}"#,
            r#"{"yaw":0,"pitch":0,"distance":121}"#,
        ] {
            let source = format!(r#"{{"sdkVersion":"0.5.0","world":{{"camera":{camera}}}}}"#);
            assert!(GamePackageDefinition::parse(&source).is_err());
        }
    }

    #[test]
    fn rejects_sdk_04_for_sdk_05_world_fields() {
        for field in [
            r#""collision":{"formatVersion":1,"triangles":[[[0,0,0],[1,0,0],[0,0,1]]]}}"#,
            r#""world":{"camera":{"yaw":0,"pitch":0,"distance":8}}}"#,
            r#""world":{"physics":{"horizontalBounds":{"minimum":[-1,-1],"maximum":[1,1]}}}}"#,
            r#""world":{"visual":{"sunRays":{"intensity":0.058,"spread":0.463}}}}"#,
        ] {
            let source = format!(r#"{{"sdkVersion":"0.4.0",{field}"#);
            let error = GamePackageDefinition::parse(&source)
                .expect_err("SDK 0.4 must not silently ignore SDK 0.5 fields");
            assert!(error.to_string().contains("require sdkVersion 0.5.0"));
        }
    }

    #[test]
    fn rejects_out_of_range_environment_controls() {
        let error = GamePackageDefinition::parse(
            r#"{"sdkVersion":"0.5.0","world":{"visual":{"daylight":{"timeOfDay":24}}}}"#,
        )
        .expect_err("24:00 is outside the documented half-open day range");
        assert!(error.to_string().contains("world.visual"));
    }

    #[test]
    fn parses_general_survival_rules_without_obby_fields() {
        let package = GamePackageDefinition::parse(
            r##"{
                "startWorld":"survival",
                "lobby":false,
                "worlds":{
                    "survival":{
                        "world":{
                            "spawn":[0,1,2],
                            "health":{"max":75,"start":50},
                            "respawn":{"mode":"spawn","delay":1.2}
                        },
                        "hazards":[{
                            "id":"deep-water",
                            "kind":"damage",
                            "position":[0,0.25,0],
                            "size":[10,0.5,10],
                            "damagePerSecond":18
                        }]
                    }
                }
            }"##,
        )
        .expect("survival package should parse");
        let world = package
            .world_entries()
            .into_iter()
            .find(|(id, _)| id == "survival")
            .expect("survival world should be present")
            .1;
        assert_eq!(world.world.health.max, 75.0);
        assert_eq!(world.world.health.start, 50.0);
        assert_eq!(world.world.respawn.mode, "spawn");
        assert_eq!(world.hazards.len(), 1);
        assert_eq!(world.hazards[0].damage_per_second, 18.0);
    }

    #[test]
    fn phase5_character_member_is_additive_and_bounded() {
        let package = GamePackageDefinition::parse(
            r##"{
                "avatars": {
                    "player": {
                        "skin": "#e8ae86",
                        "shirt": "#2d6663",
                        "character": {
                            "version": 1,
                            "body": "cuba:person.v1",
                            "face": "happy",
                            "outfit": "cuba:everyday-hoodie.v1",
                            "equipment": {"hat": "cuba:star-cap.v1"},
                            "colors": {"sole": "#f6f1e7"}
                        }
                    }
                }
            }"##,
        )
        .expect("phase 5 appearance should parse");
        let character = package
            .avatars
            .player
            .as_ref()
            .and_then(|avatar| avatar.character.as_ref())
            .expect("character member");
        assert_eq!(character.body.as_deref(), Some("cuba:person.v1"));
        assert_eq!(
            character.equipment.get("hat").map(String::as_str),
            Some("cuba:star-cap.v1")
        );
        assert!(character.bounded());
    }

    #[test]
    fn old_avatar_shape_still_parses_without_character_data() {
        let package = GamePackageDefinition::parse(
            r##"{"avatars":{"player":{"skin":"#ffffff","shirt":"#000000"}}}"##,
        )
        .expect("legacy avatar should parse");
        assert!(package.avatars.player.unwrap().character.is_none());
    }

    #[test]
    fn disabled_lobby_starts_in_the_launch_destination() {
        let package = GamePackageDefinition::parse(
            r#"{
                "lobby": false,
                "startWorld": "lobby",
                "launch": {"destinationWorld": "arena"},
                "worlds": {"arena": {}}
            }"#,
        )
        .expect("package should parse");

        assert_eq!(package.initial_world_id(), Some("arena"));
        assert_eq!(package.world_entries().len(), 2);
    }

    #[test]
    fn interaction_world_contract_parses_generic_zones() {
        let package = GamePackageDefinition::parse(
            r##"{
                "worlds": {
                    "real-game": {
                        "interactions": [{
                            "id": "blue-button",
                            "kind": "zone",
                            "label": "BLUE BUTTON",
                            "position": [-4, 0, -8],
                            "radius": 3,
                            "color": "#5bd6d0"
                        }]
                    }
                }
            }"##,
        )
        .expect("interaction contract should parse");
        let interaction = package.world_entries()[1].1.interactions[0].clone();
        assert_eq!(interaction.kind, "zone");
        assert_eq!(interaction.position(), [-4.0, 0.0, -8.0]);
        assert_eq!(interaction.label, "BLUE BUTTON");
    }

    #[test]
    fn effect_templates_are_versioned_and_composed_from_generic_nodes() {
        let package = GamePackageDefinition::parse(
            r##"{
                "effects": {
                    "version": 1,
                    "templates": {
                        "finish-flash": {
                            "duration": 1.5,
                            "nodes": [{
                                "shape": "ring",
                                "size": [2, 0.1, 1],
                                "color": "#5bd6d0",
                                "variants": [
                                    {"visibleStates": ["closed"], "opacity": 0.2},
                                    {
                                        "visibleStates": ["open"],
                                        "opacity": 1,
                                        "animation": {"pulseAmount": 0.1}
                                    }
                                ],
                                "animation": {"expandAmount": 3, "fade": true}
                            }]
                        }
                    }
                },
                "worlds": {
                    "arena": {
                        "interactions": [{"id": "finish", "visual": "finish-flash"}]
                    }
                }
            }"##,
        )
        .expect("generic effect contract should parse");

        assert_eq!(package.effects.version, crate::effects::EFFECTS_VERSION);
        let template = &package.effects.templates["finish-flash"];
        assert_eq!(template.duration, 1.5);
        assert_eq!(template.nodes[0].shape, "ring");
        assert_eq!(template.nodes[0].animation.expand_amount, 3.0);
        assert_eq!(template.nodes[0].variants[0].opacity, Some(0.2));
        assert_eq!(
            template.nodes[0].variants[1].animation.pulse_amount,
            Some(0.1)
        );
        assert_eq!(
            package.world_entries()[1].1.interactions[0]
                .visual
                .as_deref(),
            Some("finish-flash")
        );
    }
}
