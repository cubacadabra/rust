use std::collections::BTreeMap;

use serde::Deserialize;

fn default_start_world() -> String {
    "lobby".to_owned()
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
    pub(crate) worlds: BTreeMap<String, WorldDefinition>,
    #[serde(default)]
    pub(crate) avatars: AvatarSetDefinition,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsRoomDefinition {
    pub(crate) world_id: String,
    #[serde(default)]
    pub(crate) door_position: Vec<f32>,
    #[serde(default = "default_settings_proximity_radius")]
    pub(crate) proximity_radius: f32,
    pub(crate) bounds: SettingsRoomBoundsDefinition,
}

impl SettingsRoomDefinition {
    pub(crate) fn door_x(&self) -> f32 {
        self.door_position.first().copied().unwrap_or(0.0)
    }

    pub(crate) fn door_z(&self) -> f32 {
        self.door_position
            .get(2)
            .or_else(|| self.door_position.get(1))
            .copied()
            .unwrap_or(0.0)
    }

    pub(crate) fn contains(&self, x: f32, z: f32) -> bool {
        let min_x = self.bounds.min_x.min(self.bounds.max_x);
        let max_x = self.bounds.min_x.max(self.bounds.max_x);
        let min_z = self.bounds.min_z.min(self.bounds.max_z);
        let max_z = self.bounds.min_z.max(self.bounds.max_z);
        x >= min_x && x <= max_x && z >= min_z && z <= max_z
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsRoomBoundsDefinition {
    pub(crate) min_x: f32,
    pub(crate) max_x: f32,
    pub(crate) min_z: f32,
    pub(crate) max_z: f32,
}

fn default_settings_proximity_radius() -> f32 {
    7.0
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
        };
        std::iter::once(("lobby".to_owned(), lobby))
            .chain(
                self.worlds
                    .iter()
                    .map(|(id, world)| (id.clone(), world.clone())),
            )
            .collect()
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchRouteDefinition {
    pub(crate) destination_world: Option<String>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
