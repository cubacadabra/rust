use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    ops::{Deref, DerefMut},
};

use cubacadabra_engine::Engine;
use serde_json::Value;

use crate::protocol::{
    AppearanceMessage, ClientAction, ClientMovement, EngineNetworkMessage, ExperienceLaunch,
    MessageKind, MovementMessage, OutboundGameMessage, PackageHeader, PlayerNameMessage,
    PresenceMessage, RemotePlayerUpdate, RemoteRoster, SessionIdentity,
};

#[derive(Debug)]
pub struct ClientError(String);

impl ClientError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ClientError {}

#[derive(Clone, Debug)]
struct RemotePlayer {
    user_id: Option<String>,
    username: String,
    generation: u32,
    position: [f32; 3],
    yaw: f32,
    moving: bool,
    sprinting: bool,
    appearance: Option<Value>,
}

impl RemotePlayer {
    fn joined(id: &str, message: PresenceMessage) -> Self {
        Self {
            user_id: message.user_id,
            username: message.username.unwrap_or_else(|| id.to_owned()),
            generation: message.generation,
            position: [0.0; 3],
            yaw: 0.0,
            moving: false,
            sprinting: false,
            appearance: message.appearance,
        }
    }
}

/// Platform-neutral game-client session state.
///
/// Hosts provide package bytes and a WebSocket transport. This type owns the
/// engine-facing protocol, world routing, remote roster, and script outbox.
pub struct ClientSession {
    engine: Engine,
    game_id: String,
    launch_world_id: Option<String>,
    remote_players: BTreeMap<String, RemotePlayer>,
    remote_roster_dirty: bool,
    remote_sequence: u64,
    player_id: Option<String>,
    connected_world_id: Option<String>,
    pending_session_world_id: Option<String>,
    ignored_player_ids: BTreeSet<String>,
}

impl ClientSession {
    pub fn load(manifest_source: &str, script_source: &str) -> Result<Self, ClientError> {
        let header: PackageHeader = serde_json::from_str(manifest_source)
            .map_err(|error| ClientError::new(format!("manifest is not valid JSON: {error}")))?;
        if header.id.trim().is_empty() {
            return Err(ClientError::new(
                "manifest.json is missing a non-empty string id",
            ));
        }

        let mut engine = Engine::new();
        if !engine.load_package_source(manifest_source) {
            return Err(ClientError::new("the shared engine rejected manifest.json"));
        }
        if !engine.load_script_source(script_source) {
            return Err(ClientError::new(
                "the shared engine could not compile game.luau",
            ));
        }
        if engine.active_world_id().is_none() {
            return Err(ClientError::new(
                "the game manifest did not define a start world",
            ));
        }

        Ok(Self {
            engine,
            game_id: header.id,
            launch_world_id: header.launch.destination_world,
            remote_players: BTreeMap::new(),
            remote_roster_dirty: true,
            remote_sequence: 0,
            player_id: None,
            connected_world_id: None,
            pending_session_world_id: None,
            ignored_player_ids: BTreeSet::new(),
        })
    }

    pub fn game_id(&self) -> &str {
        &self.game_id
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub fn transport_connected(&mut self) {
        self.reset_remote_session();
    }

    pub fn transport_disconnected(&mut self) {
        self.reset_remote_session();
    }

    /// Requests a fresh `SetWorld` action even when the engine world has not
    /// changed. Hosts use this when resuming after an intentional socket stop;
    /// transient disconnects remain the transport's reconnect responsibility.
    pub fn request_transport(&mut self) {
        self.connected_world_id = None;
    }

    pub fn set_ignored_player_ids<I, S>(&mut self, player_ids: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let next = player_ids.into_iter().map(Into::into).collect();
        if self.ignored_player_ids != next {
            self.ignored_player_ids = next;
            self.remote_roster_dirty = true;
        }
    }

    pub fn receive_text(&mut self, source: &str) -> bool {
        let Ok(kind) = serde_json::from_str::<MessageKind>(source) else {
            return false;
        };
        match kind.kind.as_str() {
            "session_identity" => self.receive_session_identity(source),
            "player_join" => self.receive_player_join(source),
            "player_leave" => self.receive_player_leave(source),
            "appearance" => self.receive_appearance(source),
            "player_name" => self.receive_player_name(source),
            "move" => self.receive_movement(source),
            "experience_launch" => self.receive_experience_launch(source),
            "game_state" | "game_message" | "player_state" => {
                self.engine.receive_network_message_json(source)
            }
            _ => false,
        }
    }

    /// Synchronizes client state with the engine and returns work for the host
    /// transport. Call before and after stepping the engine.
    pub fn poll_actions(&mut self) -> Vec<ClientAction> {
        let mut actions = Vec::new();
        self.sync_backend_world(&mut actions);
        self.sync_remote_players();
        self.flush_engine_messages(&mut actions);
        actions
    }

    pub fn local_movement(&self, moving: bool, sprinting: bool) -> Option<ClientMovement> {
        if self.engine.active_world_id() == Some("settings") {
            return None;
        }
        let snapshot = self.engine.snapshot();
        if snapshot.len() < 3 {
            return None;
        }
        Some(ClientMovement {
            position: [snapshot[0], snapshot[1], snapshot[2]],
            yaw: self.engine.player_facing_yaw(),
            moving,
            sprinting,
            respawn_event_id: self.engine.player_respawn_event_id(),
        })
    }

    fn receive_session_identity(&mut self, source: &str) -> bool {
        let Ok(message) = serde_json::from_str::<SessionIdentity>(source) else {
            return false;
        };
        self.player_id = Some(message.id);
        true
    }

    fn receive_player_join(&mut self, source: &str) -> bool {
        let Ok(message) = serde_json::from_str::<PresenceMessage>(source) else {
            return false;
        };
        if self.player_id.as_deref() == Some(message.id.as_str()) {
            return true;
        }
        let id = message.id.clone();
        self.remote_players
            .insert(id.clone(), RemotePlayer::joined(&id, message));
        self.remote_roster_dirty = true;
        true
    }

    fn receive_player_leave(&mut self, source: &str) -> bool {
        let Ok(message) = serde_json::from_str::<SessionIdentity>(source) else {
            return false;
        };
        if self.remote_players.remove(&message.id).is_some() {
            self.remote_roster_dirty = true;
        }
        true
    }

    fn receive_appearance(&mut self, source: &str) -> bool {
        let Ok(message) = serde_json::from_str::<AppearanceMessage>(source) else {
            return false;
        };
        if let Some(player) = self.remote_players.get_mut(&message.id) {
            player.appearance = message.appearance;
            self.remote_roster_dirty = true;
        }
        true
    }

    fn receive_player_name(&mut self, source: &str) -> bool {
        let Ok(message) = serde_json::from_str::<PlayerNameMessage>(source) else {
            return false;
        };
        if let Some(player) = self.remote_players.get_mut(&message.id) {
            player.username = message.username;
            self.remote_roster_dirty = true;
        }
        true
    }

    fn receive_movement(&mut self, source: &str) -> bool {
        let Ok(message) = serde_json::from_str::<MovementMessage>(source) else {
            return false;
        };
        if !message.is_finite() {
            return false;
        }
        if self.player_id.as_deref() == Some(message.id.as_str()) {
            if message.corrected {
                self.engine
                    .reconcile_player([message.x, message.y, message.z], message.yaw);
            }
            return true;
        }
        let player = self
            .remote_players
            .entry(message.id.clone())
            .or_insert_with(|| RemotePlayer {
                user_id: None,
                username: message.id.clone(),
                generation: 0,
                position: [0.0; 3],
                yaw: 0.0,
                moving: false,
                sprinting: false,
                appearance: None,
            });
        player.position = [message.x, message.y, message.z];
        player.yaw = message.yaw;
        player.moving = message.moving;
        player.sprinting = message.sprinting;
        if message.generation != 0 {
            player.generation = message.generation;
        }
        self.remote_roster_dirty = true;
        true
    }

    fn receive_experience_launch(&mut self, source: &str) -> bool {
        let Ok(message) = serde_json::from_str::<ExperienceLaunch>(source) else {
            return false;
        };
        let Some(player_id) = self.player_id.as_deref() else {
            return true;
        };
        if !message.player_ids.iter().any(|id| id == player_id) {
            return true;
        }
        self.pending_session_world_id = message.session_world_id;
        self.launch_world_id
            .as_deref()
            .is_some_and(|world_id| self.engine.start_world_by_id(world_id))
    }

    fn reset_remote_session(&mut self) {
        self.remote_players.clear();
        self.remote_roster_dirty = true;
        self.remote_sequence = 0;
        self.player_id = None;
        self.engine.reset_remote_session();
    }

    fn sync_backend_world(&mut self, actions: &mut Vec<ClientAction>) {
        let Some(world_id) = self.engine.active_world_id() else {
            return;
        };
        let network_world_id = if world_id == "settings" {
            "lobby".to_owned()
        } else if self.launch_world_id.as_deref() == Some(world_id) {
            self.pending_session_world_id
                .clone()
                .unwrap_or_else(|| world_id.to_owned())
        } else {
            world_id.to_owned()
        };
        if self.connected_world_id.as_deref() == Some(network_world_id.as_str()) {
            return;
        }
        self.connected_world_id = Some(network_world_id.clone());
        self.reset_remote_session();
        actions.push(ClientAction::SetWorld(network_world_id));
    }

    fn sync_remote_players(&mut self) {
        if !self.remote_roster_dirty {
            return;
        }
        let Some(world_id) = self.engine.active_world_id() else {
            return;
        };
        self.remote_sequence = self.remote_sequence.saturating_add(1);
        let players = self
            .remote_players
            .iter()
            .filter(|(id, player)| !self.player_is_ignored(id, player))
            .map(|(id, player)| RemotePlayerUpdate {
                id,
                username: &player.username,
                generation: player.generation,
                position: player.position,
                yaw: player.yaw,
                moving: player.moving,
                sprinting: player.sprinting,
                appearance: player.appearance.as_ref(),
            })
            .collect();
        let roster = RemoteRoster {
            version: 1,
            sequence: self.remote_sequence,
            world_id,
            players,
        };
        let Ok(source) = serde_json::to_string(&roster) else {
            return;
        };
        if self.engine.apply_remote_update_json(&source) {
            self.remote_roster_dirty = false;
        }
    }

    fn player_is_ignored(&self, id: &str, player: &RemotePlayer) -> bool {
        self.ignored_player_ids.contains(id)
            || player
                .user_id
                .as_ref()
                .is_some_and(|user_id| self.ignored_player_ids.contains(user_id))
    }

    fn flush_engine_messages(&mut self, actions: &mut Vec<ClientAction>) {
        while let Some(source) = self.engine.poll_network_message_json() {
            let Ok(message) = serde_json::from_str::<EngineNetworkMessage>(&source) else {
                continue;
            };
            let expected_sequence = message
                .expected_sequence
                .filter(|value| *value <= u32::MAX as u64);
            let kind = if expected_sequence.is_some() {
                "game_state_compare_set"
            } else if message.retained {
                "game_state_set"
            } else {
                "game_message"
            };
            let outbound = OutboundGameMessage {
                kind,
                channel: &message.channel,
                payload: &message.payload,
                expected_sequence,
            };
            if let Ok(source) = serde_json::to_string(&outbound) {
                actions.push(ClientAction::SendText(source));
            }
        }
    }
}

impl Deref for ClientSession {
    type Target = Engine;

    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

impl DerefMut for ClientSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::ClientSession;

    const MANIFEST: &str = r#"{
        "id":"test-game",
        "startWorld":"lobby",
        "launch":{"destinationWorld":"arena"},
        "worlds":{"arena":{}}
    }"#;

    const SCRIPT: &str = "return {}";

    #[test]
    fn rejects_invalid_protocol_messages() {
        let mut client = ClientSession::load(MANIFEST, SCRIPT).expect("client");
        assert!(!client.receive_text("not json"));
        assert!(!client.receive_text(r#"{"type":"move","id":"p","x":null}"#));
    }

    #[test]
    fn routes_launch_group_to_session_world() {
        let mut client = ClientSession::load(MANIFEST, SCRIPT).expect("client");
        assert!(client.receive_text(r#"{"type":"session_identity","id":"me"}"#));
        assert!(client.receive_text(
            r#"{"type":"experience_launch","playerIds":["me"],"sessionWorldId":"arena:7"}"#,
        ));
        assert_eq!(client.active_world_id(), Some("arena"));
        let actions = client.poll_actions();
        assert!(matches!(
            actions.first(),
            Some(crate::ClientAction::SetWorld(world)) if world == "arena:7"
        ));
    }

    #[test]
    fn projects_remote_players_and_filters_blocked_accounts() {
        let mut client = ClientSession::load(MANIFEST, SCRIPT).expect("client");
        client.poll_actions();
        client.transport_connected();
        assert!(client.receive_text(r#"{"type":"session_identity","id":"me"}"#));
        assert!(client.receive_text(
            r#"{"type":"player_join","id":"remote","user_id":"account-7","username":"Ada"}"#,
        ));
        assert!(client.receive_text(
            r#"{"type":"move","id":"remote","x":1,"y":2,"z":3,"yaw":0.5,"moving":true}"#,
        ));
        client.poll_actions();
        let remote = client.remote_players.get("remote").expect("remote player");
        assert!(!client.player_is_ignored("remote", remote));

        client.set_ignored_player_ids(["account-7"]);
        let remote = client.remote_players.get("remote").expect("remote player");
        assert!(client.player_is_ignored("remote", remote));
    }

    #[test]
    fn transient_disconnect_preserves_route_until_host_requests_transport() {
        let mut client = ClientSession::load(MANIFEST, SCRIPT).expect("client");
        assert!(matches!(
            client.poll_actions().first(),
            Some(crate::ClientAction::SetWorld(world)) if world == "lobby"
        ));
        client.transport_disconnected();
        assert!(client.poll_actions().is_empty());
        client.request_transport();
        assert!(matches!(
            client.poll_actions().first(),
            Some(crate::ClientAction::SetWorld(world)) if world == "lobby"
        ));
    }
}
