//! Portable host adapter for trusted, game-owned Luau authority rules.
//!
//! This is the execution seam shared by the Cloudflare/Wasm host and a future
//! native headless server. Authentication, authoritative world facts, durable
//! storage, and broadcasting remain responsibilities of the host.

use serde_json::{Value, json};

use crate::authority::{AuthorityBoundary, Command, MAX_AUTHORITY_COMMAND_BYTES};
use crate::scripting::authority::LuauAuthorityRules;

const MAX_INITIAL_STATE_BYTES: usize = 1024 * 1024;

/// Runs one trusted game ruleset against a host-owned durable snapshot.
///
/// Each command restores from the snapshot supplied by the host and returns a
/// candidate snapshot. The runtime therefore does not advance hidden
/// in-memory gameplay state if the host's durable write fails.
pub struct ServerAuthority {
    rules_source: String,
}

impl ServerAuthority {
    /// Loads and validates one pinned game-owned ruleset.
    pub fn load(rules_source: &str) -> Result<Self, String> {
        LuauAuthorityRules::load(rules_source)?;
        Ok(Self {
            rules_source: rules_source.to_owned(),
        })
    }

    /// Creates the initial checkpoint from host-created state. The state must
    /// include only host-trusted facts.
    pub fn initial_snapshot_json(&self, initial_state_json: &str) -> Result<String, String> {
        if initial_state_json.len() > MAX_INITIAL_STATE_BYTES {
            return Err("authority initial state exceeds the 1 MiB limit".to_owned());
        }
        let initial_state: Value = serde_json::from_str(initial_state_json)
            .map_err(|error| format!("invalid authority initial state: {error}"))?;
        if !initial_state.is_object() {
            return Err("authority initial state must be a JSON object".to_owned());
        }

        AuthorityBoundary::new(LuauAuthorityRules::load(&self.rules_source)?, initial_state)
            .snapshot_json()
    }

    /// Executes a client intent against the supplied durable checkpoint as
    /// the authenticated actor supplied by the trusted transport. The caller
    /// must persist the returned candidate snapshot before publishing events.
    pub fn execute_json(
        &self,
        snapshot_json: &str,
        actor_id: &str,
        command_json: &str,
    ) -> Result<String, String> {
        if command_json.len() > MAX_AUTHORITY_COMMAND_BYTES {
            return Err("authority command exceeds the 16 KiB limit".to_owned());
        }
        let command: Command = serde_json::from_str(command_json)
            .map_err(|error| format!("invalid authority command: {error}"))?;
        let mut boundary = AuthorityBoundary::restore_json(
            LuauAuthorityRules::load(&self.rules_source)?,
            snapshot_json,
        )?;
        let execution = boundary.execute(actor_id, command);
        let snapshot = boundary.snapshot_json()?;
        let response = json!({
            "outcome": execution.outcome,
            "replayed": execution.replayed,
            "snapshot": snapshot,
        });
        serde_json::to_string(&response).map_err(|error| error.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    use super::ServerAuthority;

    /// Synchronous WebAssembly interface for a JS host such as a Durable
    /// Object. The JS host must derive `authenticated_actor_id` from its
    /// connection and load `snapshot_json` from durable storage; neither value
    /// is accepted from an untrusted command payload.
    #[wasm_bindgen]
    pub struct WasmServerAuthority {
        inner: ServerAuthority,
    }

    #[wasm_bindgen]
    impl WasmServerAuthority {
        #[wasm_bindgen(constructor)]
        pub fn load(rules_source: &str) -> Result<Self, JsValue> {
            ServerAuthority::load(rules_source)
                .map(|inner| Self { inner })
                .map_err(js_error)
        }

        pub fn initial_snapshot_json(&self, initial_state_json: &str) -> Result<String, JsValue> {
            self.inner
                .initial_snapshot_json(initial_state_json)
                .map_err(js_error)
        }

        pub fn execute_json(
            &self,
            snapshot_json: &str,
            authenticated_actor_id: &str,
            command_json: &str,
        ) -> Result<String, JsValue> {
            self.inner
                .execute_json(snapshot_json, authenticated_actor_id, command_json)
                .map_err(js_error)
        }
    }

    fn js_error(error: String) -> JsValue {
        JsValue::from_str(&error)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::ServerAuthority;
    use crate::authority::CommandOutcome;

    const MAZE_RULES: &str = include_str!("../../examples/maze-101/src/server.luau");

    fn initial_state() -> Value {
        json!({
            "round": {
                "phase": "active",
                "elapsedSeconds": 12.0,
                "timeLimitSeconds": 180.0
            },
            "maze": {
                "coinRadius": 2.0,
                "coins": {
                    "maze-coin-01": { "position": { "x": 0.0, "y": 1.0, "z": 0.0 } }
                },
                "finish": {
                    "position": { "x": 10.0, "y": 1.0, "z": 0.0 },
                    "radius": 2.5
                }
            },
            "players": {
                "alice": {
                    "position": { "x": 0.5, "y": 1.0, "z": 0.0 },
                    "status": "running",
                    "coinCount": 0,
                    "collectedCoins": {}
                },
                "bob": {
                    "position": { "x": 30.0, "y": 1.0, "z": 0.0 },
                    "status": "running",
                    "coinCount": 0,
                    "collectedCoins": {}
                }
            }
        })
    }

    #[test]
    fn runs_packaged_maze_rules_and_returns_state_with_retry_receipt() {
        let authority =
            ServerAuthority::load(MAZE_RULES).expect("trusted Maze 101 rules should load");
        let base_snapshot = authority
            .initial_snapshot_json(&serde_json::to_string(&initial_state()).unwrap())
            .expect("host-created state should checkpoint");
        let intent = r#"{"requestId":"alice-coin-1","name":"collect_coin","payload":{"coinId":"maze-coin-01","actorId":"bob","position":{"x":30,"y":1,"z":0}}}"#;

        let response: Value = serde_json::from_str(
            &authority
                .execute_json(&base_snapshot, "alice", intent)
                .expect("host-bound command should execute"),
        )
        .unwrap();
        assert_eq!(response["outcome"]["status"], "accepted");
        assert_eq!(
            response["outcome"]["state"]["players"]["alice"]["coinCount"],
            1
        );
        assert_eq!(
            response["outcome"]["state"]["players"]["bob"]["coinCount"],
            0
        );
        assert_eq!(response["outcome"]["events"][0]["actorId"], "alice");

        // If durable persistence fails, executing against the old snapshot is
        // a fresh candidate and leaves no advanced runtime state behind.
        let retried_before_commit: Value = serde_json::from_str(
            &authority
                .execute_json(&base_snapshot, "alice", intent)
                .expect("uncommitted retry should be reproducible"),
        )
        .unwrap();
        assert_eq!(retried_before_commit["replayed"], false);
        assert_eq!(retried_before_commit, response);

        let committed_snapshot = response["snapshot"].as_str().unwrap();
        let retried: Value = serde_json::from_str(
            &authority
                .execute_json(committed_snapshot, "alice", intent)
                .expect("retry should return the cached result"),
        )
        .unwrap();
        assert_eq!(retried["replayed"], true);
        assert_eq!(
            retried["outcome"]["state"]["players"]["alice"]["coinCount"],
            1
        );
    }

    #[test]
    fn runtime_rejects_untrusted_initial_state_and_invalid_json() {
        let authority = ServerAuthority::load(MAZE_RULES).unwrap();
        assert!(authority.initial_snapshot_json("[]").is_err());
        assert!(authority.initial_snapshot_json("not-json").is_err());
        assert!(authority.execute_json("{}", "alice", "not-json").is_err());
    }

    #[test]
    fn host_actor_binding_overrides_actor_claims_in_the_intent() {
        let authority = ServerAuthority::load(MAZE_RULES).unwrap();
        let snapshot = authority
            .initial_snapshot_json(&serde_json::to_string(&initial_state()).unwrap())
            .unwrap();
        let response: Value = serde_json::from_str(
            &authority
                .execute_json(
                    &snapshot,
                    "bob",
                    r#"{"requestId":"bob-coin-1","name":"collect_coin","payload":{"coinId":"maze-coin-01","actorId":"alice"}}"#,
                )
                .unwrap(),
        )
        .unwrap();
        let outcome: CommandOutcome = serde_json::from_value(response["outcome"].clone()).unwrap();
        assert!(matches!(outcome, CommandOutcome::Rejected { .. }));
        assert_eq!(response["snapshot"].as_str().is_some(), true);
        assert_eq!(
            response["outcome"]["rejection"]["code"],
            "outside_pickup_radius"
        );
    }
}
