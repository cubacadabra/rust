use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum ClientAction {
    SetWorld(String),
    SendText(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientMovement {
    pub position: [f32; 3],
    pub yaw: f32,
    pub moving: bool,
    pub sprinting: bool,
    pub respawn_event_id: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageHeader {
    pub id: String,
    #[serde(default)]
    pub launch: LaunchRoute,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaunchRoute {
    pub destination_world: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MessageKind {
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionIdentity {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PresenceMessage {
    pub id: String,
    #[serde(default, alias = "user_id")]
    pub user_id: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub generation: u32,
    #[serde(default)]
    pub appearance: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlayerNameMessage {
    pub id: String,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AppearanceMessage {
    pub id: String,
    #[serde(default)]
    pub appearance: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MovementMessage {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    #[serde(default)]
    pub moving: bool,
    #[serde(default)]
    pub sprinting: bool,
    #[serde(default)]
    pub generation: u32,
    #[serde(default)]
    pub corrected: bool,
}

impl MovementMessage {
    pub(crate) fn is_finite(&self) -> bool {
        [self.x, self.y, self.z, self.yaw]
            .into_iter()
            .all(f32::is_finite)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExperienceLaunch {
    #[serde(default)]
    pub player_ids: Vec<String>,
    #[serde(default)]
    pub session_world_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EngineNetworkMessage {
    pub channel: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub retained: bool,
    #[serde(default)]
    pub expected_sequence: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OutboundGameMessage<'a> {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub channel: &'a str,
    pub payload: &'a Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sequence: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteRoster<'a> {
    pub version: u8,
    pub sequence: u64,
    pub world_id: &'a str,
    pub players: Vec<RemotePlayerUpdate<'a>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemotePlayerUpdate<'a> {
    pub id: &'a str,
    pub username: &'a str,
    pub generation: u32,
    pub position: [f32; 3],
    pub yaw: f32,
    pub moving: bool,
    pub sprinting: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<&'a Value>,
}
