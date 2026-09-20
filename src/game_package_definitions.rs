use super::*;

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
            && package._sdk_version.as_deref() != Some(LEGACY_CURRENT_SDK_VERSION)
            && package._sdk_version.as_deref() != Some(CURRENT_SDK_VERSION)
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "terrain operations require sdkVersion {TERRAIN_SDK_VERSION}, {LEGACY_CURRENT_SDK_VERSION}, or {CURRENT_SDK_VERSION}"
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
        if has_sdk_05_fields
            && package._sdk_version.as_deref() != Some(LEGACY_CURRENT_SDK_VERSION)
            && package._sdk_version.as_deref() != Some(CURRENT_SDK_VERSION)
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "collision, world.camera, world.physics.horizontalBounds, and world.visual environment controls require sdkVersion {LEGACY_CURRENT_SDK_VERSION} or {CURRENT_SDK_VERSION}"
                ),
            )));
        }
        let has_non_uniform_mesh_scale = package.worlds.values().any(|world| {
            world
                .decorations
                .iter()
                .any(|decoration| decoration.scale3.is_some())
        });
        if has_non_uniform_mesh_scale
            && package._sdk_version.as_deref() != Some(CURRENT_SDK_VERSION)
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("mesh scale3 requires sdkVersion {CURRENT_SDK_VERSION}"),
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
    pub(crate) scale3: Option<[f32; 3]>,
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
        self.scale3.unwrap_or([self.scale; 3])
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
