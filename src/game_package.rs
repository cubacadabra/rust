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
            interactions: Vec::new(),
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
    pub(crate) interactions: Vec<InteractionDefinition>,
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
