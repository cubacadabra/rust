use std::collections::BTreeMap;

use serde::Deserialize;

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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GamePackageDefinition {
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
        serde_json::from_str(source)
    }

    pub(crate) fn world_entries(&self) -> Vec<(String, WorldDefinition)> {
        let lobby = WorldDefinition {
            palette: self.palette.clone(),
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
    #[serde(default)]
    pub(crate) spawn: Vec<f32>,
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
}

impl Default for WorldSettingsDefinition {
    fn default() -> Self {
        Self {
            ground_size: default_ground_size(),
            grid_size: default_grid_size(),
            grid_divisions: default_grid_divisions(),
            spawn: vec![0.0, 0.0, 0.0],
            show_spawn_pad: true,
            clouds: Vec::new(),
            physics: PhysicsDefinition::default(),
            health: HealthDefinition::default(),
            respawn: RespawnDefinition::default(),
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
        }
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
    #[serde(default = "default_true")]
    pub(crate) outline: bool,
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
            .chain(self.face.iter())
            .chain(self.outfit.iter())
            .chain(self.equipment.keys())
            .chain(self.equipment.values())
            .chain(self.colors.keys())
            .chain(self.colors.values())
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
        assert_eq!(lobby.world.clouds[0].position(), [4.0, 5.0, 6.0]);
        assert_eq!(lobby.launch_pads[0].label, "SUN COURT");
        assert!(!lobby.blocks[0].outline);
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
