//! Sandboxed Luau adapter for game-owned authority rules.
//!
//! This is a host-side rules runner, not a client capability. The host must
//! authenticate the actor and supply the authoritative state before invoking
//! it. Game rules receive explicit state and command values and return a full
//! state transition; they do not get networking, storage, filesystem, or
//! renderer APIs.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Value, json};

use super::{ExecutionBudget, execute_with_budget, json_to_lua, lua, lua_value_to_json};
use crate::authority::{AuthenticatedCommand, CommandHandler, EventSpec, Rejection, Simulation};

const AUTHORITY_RULES_BUDGET: u32 = 100_000;
const AUTHORITY_RULES_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const MAX_AUTHORITY_RULES_SOURCE_BYTES: usize = 1024 * 1024;

/// Game-authored rules running in a sandboxed Luau VM.
///
/// The module must return `validate_command(state, command)` and
/// `simulate_command(state, command)`. Validation returns `nil` for acceptance
/// or a stable rejection-code string. Simulation returns
/// `{ state = nextState, events = { { name = ..., payload = ... } } }`.
///
/// Rules are expected to be deterministic functions of their explicit inputs.
/// Persistent gameplay state belongs in the returned state, not hidden module
/// globals, so a host can snapshot and resume it.
pub struct LuauAuthorityRules {
    lua: lua::Lua,
    validate_command: lua::Function,
    simulate_command: lua::Function,
    execution_budget: Rc<RefCell<ExecutionBudget>>,
    last_error: RefCell<Option<String>>,
}

impl LuauAuthorityRules {
    /// Load a game-owned server rules module in a sandboxed Luau VM.
    ///
    /// The caller is responsible for treating `source` as trusted package
    /// content and for passing only host-authenticated identities and
    /// authoritative world state to [`CommandHandler`].
    pub fn load(source: &str) -> Result<Self, String> {
        if source.len() > MAX_AUTHORITY_RULES_SOURCE_BYTES {
            return Err("authority rules source exceeds the 1 MiB limit".to_owned());
        }
        let lua = lua::Lua::new();
        lua.set_memory_limit(AUTHORITY_RULES_MEMORY_BYTES)
            .map_err(|error| error.to_string())?;
        let execution_budget = Rc::new(RefCell::new(ExecutionBudget::default()));
        let interrupt_budget = Rc::clone(&execution_budget);
        lua.set_interrupt(move |_| {
            let mut budget = interrupt_budget.borrow_mut();
            if !budget.active {
                return Ok(lua::VmState::Continue);
            }
            budget.safepoints = budget.safepoints.saturating_add(1);
            if budget.safepoints >= AUTHORITY_RULES_BUDGET {
                budget.exhausted = true;
                Err(lua::Error::RuntimeError(
                    "authority rules execution budget exceeded".to_owned(),
                ))
            } else {
                Ok(lua::VmState::Continue)
            }
        });
        lua.sandbox(true).map_err(|error| error.to_string())?;
        let module: lua::Table = execute_with_budget(&execution_budget, || {
            lua.load(source).set_name("authority.luau").eval()
        })
        .map_err(|error| error.to_string())?;
        let validate_command = module
            .get::<Option<lua::Function>>("validate_command")
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "authority module must define validate_command".to_owned())?;
        let simulate_command = module
            .get::<Option<lua::Function>>("simulate_command")
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "authority module must define simulate_command".to_owned())?;

        Ok(Self {
            lua,
            validate_command,
            simulate_command,
            execution_budget,
            last_error: RefCell::new(None),
        })
    }

    #[allow(dead_code)]
    pub fn last_error(&self) -> Option<String> {
        self.last_error.borrow().clone()
    }

    fn call<T>(&self, operation: impl FnOnce() -> lua::Result<T>) -> Result<T, Rejection> {
        match execute_with_budget(&self.execution_budget, operation) {
            Ok(result) => {
                *self.last_error.borrow_mut() = None;
                Ok(result)
            }
            Err(error) => {
                *self.last_error.borrow_mut() = Some(error.to_string());
                Err(Rejection::new("authority_rules_execution_failed"))
            }
        }
    }
}

impl CommandHandler for LuauAuthorityRules {
    fn validate(&self, state: &Value, command: &AuthenticatedCommand) -> Result<(), Rejection> {
        let state = json_to_lua(&self.lua, state, 0)
            .map_err(|_| Rejection::new("authority_rules_input_invalid"))?;
        let command = json_to_lua(&self.lua, &command_json(command), 0)
            .map_err(|_| Rejection::new("authority_rules_input_invalid"))?;
        let result: lua::Value = self.call(|| self.validate_command.call((state, command)))?;

        match result {
            lua::Value::Nil => Ok(()),
            lua::Value::String(code) => {
                let code = code.to_string_lossy();
                if valid_rejection_code(&code) {
                    Err(Rejection::new(code))
                } else {
                    Err(Rejection::new("authority_rules_invalid_rejection"))
                }
            }
            _ => Err(Rejection::new("authority_rules_invalid_rejection")),
        }
    }

    fn simulate(
        &self,
        state: &Value,
        command: &AuthenticatedCommand,
    ) -> Result<Simulation, Rejection> {
        let state = json_to_lua(&self.lua, state, 0)
            .map_err(|_| Rejection::new("authority_rules_input_invalid"))?;
        let command = json_to_lua(&self.lua, &command_json(command), 0)
            .map_err(|_| Rejection::new("authority_rules_input_invalid"))?;
        let result: lua::Table = self
            .call(|| self.simulate_command.call((state, command)))
            .map_err(|_| Rejection::new("authority_rules_invalid_simulation"))?;

        let next_state: lua::Value = result
            .get("state")
            .map_err(|_| Rejection::new("authority_rules_invalid_simulation"))?;
        let state = lua_value_to_json(next_state, 0)
            .map_err(|_| Rejection::new("authority_rules_invalid_simulation"))?;
        if !state.is_object() {
            return Err(Rejection::new("authority_rules_invalid_simulation"));
        }

        let events: lua::Table = result
            .get("events")
            .map_err(|_| Rejection::new("authority_rules_invalid_simulation"))?;
        let event_count = events.raw_len();
        if event_count > crate::authority::MAX_AUTHORITY_EVENTS_PER_COMMAND {
            return Err(Rejection::new("authority_rules_invalid_simulation"));
        }
        let mut parsed_events = Vec::with_capacity(event_count);
        for index in 1..=event_count {
            let event: lua::Table = events
                .get(index)
                .map_err(|_| Rejection::new("authority_rules_invalid_simulation"))?;
            let name: String = event
                .get("name")
                .map_err(|_| Rejection::new("authority_rules_invalid_simulation"))?;
            let payload: lua::Value = event
                .get("payload")
                .map_err(|_| Rejection::new("authority_rules_invalid_simulation"))?;
            let payload = lua_value_to_json(payload, 0)
                .map_err(|_| Rejection::new("authority_rules_invalid_simulation"))?;
            if !valid_event_name(&name) {
                return Err(Rejection::new("authority_rules_invalid_simulation"));
            }
            parsed_events.push(EventSpec::new(name, payload));
        }

        Ok(Simulation::new(state, parsed_events))
    }
}

fn command_json(command: &AuthenticatedCommand) -> Value {
    json!({
        "requestId": command.request_id,
        "actorId": command.actor_id,
        "name": command.name,
        "payload": command.payload,
    })
}

fn valid_rejection_code(code: &str) -> bool {
    (1..=96).contains(&code.len())
        && code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_event_name(name: &str) -> bool {
    (1..=96).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::authority::{
        AuthenticatedCommand, AuthorityBoundary, Command, CommandHandler, CommandOutcome,
    };

    use super::LuauAuthorityRules;

    fn rules() -> LuauAuthorityRules {
        LuauAuthorityRules::load(
            r#"
                local Rules = {}

                function Rules.validate_command(state, command)
                    if command.name ~= "collect" then
                        return "unsupported_command"
                    end
                    local player = state.players[command.actorId]
                    if player == nil then
                        return "player_not_in_round"
                    end
                    local coinId = command.payload.coinId
                    if state.coins[coinId] == nil then
                        return "unknown_coin"
                    end
                    if player.coins[coinId] then
                        return "already_collected"
                    end
                    return nil
                end

                function Rules.simulate_command(state, command)
                    local nextState = table.clone(state)
                    nextState.players = table.clone(state.players)
                    local player = table.clone(state.players[command.actorId])
                    player.coins = table.clone(player.coins)
                    player.coins[command.payload.coinId] = true
                    player.coinCount += 1
                    nextState.players[command.actorId] = player
                    return {
                        state = nextState,
                        events = {
                            {
                                name = "coin_collected",
                                payload = { coinId = command.payload.coinId },
                            },
                        },
                    }
                end

                return Rules
            "#,
        )
        .expect("rules module must load")
    }

    fn initial_state() -> serde_json::Value {
        json!({
            "coins": { "coin-01": true },
            "players": {
                "alice": { "coinCount": 0, "coins": {} },
                "bob": { "coinCount": 0, "coins": {} }
            }
        })
    }

    fn maze_rules() -> LuauAuthorityRules {
        LuauAuthorityRules::load(include_str!("../../../examples/maze-101/src/server.luau"))
            .expect("Maze 101 trusted rules should load")
    }

    #[test]
    fn configured_external_authority_rules_compile() {
        let Ok(path) = std::env::var("CUBACADABRA_TEST_AUTHORITY_RULES") else {
            return;
        };
        let source = std::fs::read_to_string(&path)
            .expect("configured authority rules package entry should exist");
        LuauAuthorityRules::load(&source)
            .expect("configured authority rules package entry should load");
    }

    #[test]
    fn luau_rules_receive_authenticated_actor_and_keep_player_round_state_separate() {
        let mut authority = AuthorityBoundary::new(rules(), initial_state());
        let command = Command::new(
            "alice-coin-1",
            "collect",
            json!({ "coinId": "coin-01", "actorId": "bob" }),
        );

        let outcome = authority.execute("alice", command).outcome;
        let CommandOutcome::Accepted { state, events, .. } = outcome else {
            panic!("Alice's valid pickup should be accepted");
        };

        assert_eq!(state["players"]["alice"]["coinCount"], 1);
        assert_eq!(state["players"]["bob"]["coinCount"], 0);
        assert_eq!(events[0].actor_id, "alice");
        assert_eq!(events[0].payload["coinId"], "coin-01");
    }

    #[test]
    fn luau_rules_keep_coin_progress_independent_per_player_and_reject_duplicates() {
        let mut authority = AuthorityBoundary::new(rules(), initial_state());
        let first = authority.execute(
            "alice",
            Command::new("alice-coin-1", "collect", json!({ "coinId": "coin-01" })),
        );
        assert!(matches!(first.outcome, CommandOutcome::Accepted { .. }));

        let bob = authority.execute(
            "bob",
            Command::new("bob-coin-1", "collect", json!({ "coinId": "coin-01" })),
        );
        assert!(matches!(bob.outcome, CommandOutcome::Accepted { .. }));

        let duplicate = authority.execute(
            "alice",
            Command::new("alice-coin-2", "collect", json!({ "coinId": "coin-01" })),
        );
        assert!(matches!(
            duplicate.outcome,
            CommandOutcome::Rejected { ref rejection, .. }
                if rejection.code == "already_collected"
        ));
        assert_eq!(authority.state()["players"]["alice"]["coinCount"], 1);
        assert_eq!(authority.state()["players"]["bob"]["coinCount"], 1);
    }

    #[test]
    fn luau_rules_require_both_callbacks_and_bound_execution() {
        let invalid = LuauAuthorityRules::load("return { validate_command = function() end }");
        assert!(matches!(invalid, Err(error) if error.contains("simulate_command")));

        let looping = LuauAuthorityRules::load(
            r#"
                return {
                    validate_command = function() while true do end end,
                    simulate_command = function() return {} end,
                }
            "#,
        )
        .unwrap();
        let state = json!({ "players": { "alice": {} } });
        let command = AuthenticatedCommand {
            request_id: "loop".to_owned(),
            actor_id: "alice".to_owned(),
            name: "collect".to_owned(),
            payload: json!({}),
        };
        let validation = looping.validate(&state, &command);
        assert!(matches!(
            validation,
            Err(ref rejection) if rejection.code == "authority_rules_execution_failed"
        ));
        assert!(
            looping
                .last_error()
                .is_some_and(|error| error.contains("budget exceeded"))
        );

        let mut authority = AuthorityBoundary::new(
            LuauAuthorityRules::load(
                r#"return {
                    validate_command = function() while true do end end,
                    simulate_command = function() return {} end,
                }"#,
            )
            .unwrap(),
            state,
        );
        let result = authority.execute("alice", Command::new("loop", "collect", json!({})));
        assert!(matches!(
            result.outcome,
            CommandOutcome::Rejected { ref rejection, .. }
                if rejection.code == "authority_rules_execution_failed"
        ));
        assert!(authority.state()["players"]["alice"].is_object());
    }

    #[test]
    fn maze_rules_check_trusted_positions_and_keep_each_runner_independent() {
        let mut state = json!({
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
                    "position": { "x": 0.0, "y": 1.0, "z": 0.0 },
                    "status": "running",
                    "coinCount": 0,
                    "collectedCoins": {}
                }
            }
        });
        let original = state.clone();
        let mut authority = AuthorityBoundary::new(maze_rules(), state.clone());

        let alice_command = Command::new(
            "alice-coin-1",
            "collect_coin",
            json!({ "coinId": "maze-coin-01", "actorId": "bob", "position": [999, 999, 999] }),
        );
        let alice_pickup = authority.execute("alice", alice_command.clone());
        assert!(matches!(
            alice_pickup.outcome,
            CommandOutcome::Accepted { .. }
        ));
        assert_eq!(authority.state()["players"]["alice"]["coinCount"], 1);
        assert_eq!(authority.state()["players"]["bob"]["coinCount"], 0);

        let snapshot = authority
            .snapshot_json()
            .expect("round state and retry receipt should checkpoint together");
        authority = AuthorityBoundary::restore_json(maze_rules(), &snapshot)
            .expect("round state and retry receipt should restore together");
        let reconnect_retry = authority.execute("alice", alice_command);
        assert!(reconnect_retry.replayed);
        assert_eq!(authority.state()["players"]["alice"]["coinCount"], 1);

        let duplicate = authority.execute(
            "alice",
            Command::new(
                "alice-coin-2",
                "collect_coin",
                json!({ "coinId": "maze-coin-01" }),
            ),
        );
        assert!(matches!(
            duplicate.outcome,
            CommandOutcome::Rejected { ref rejection, .. }
                if rejection.code == "already_collected"
        ));

        let bob_pickup = authority.execute(
            "bob",
            Command::new(
                "bob-coin-1",
                "collect_coin",
                json!({ "coinId": "maze-coin-01" }),
            ),
        );
        assert!(matches!(
            bob_pickup.outcome,
            CommandOutcome::Accepted { .. }
        ));
        assert_eq!(authority.state()["players"]["alice"]["coinCount"], 1);
        assert_eq!(authority.state()["players"]["bob"]["coinCount"], 1);

        state["players"]["alice"]["position"] = json!({ "x": 30.0, "y": 1.0, "z": 0.0 });
        let mut far_away = AuthorityBoundary::new(maze_rules(), state.clone());
        let forged = far_away.execute(
            "alice",
            Command::new(
                "alice-forged-coin",
                "collect_coin",
                json!({ "coinId": "maze-coin-01", "position": { "x": 0.0, "y": 1.0, "z": 0.0 } }),
            ),
        );
        assert!(matches!(
            forged.outcome,
            CommandOutcome::Rejected { ref rejection, .. }
                if rejection.code == "outside_pickup_radius"
        ));
        assert_eq!(far_away.state()["players"]["alice"]["coinCount"], 0);
        assert_eq!(original["players"]["alice"]["coinCount"], 0);
    }

    #[test]
    fn finishing_and_timeout_are_per_round_rules_and_block_later_pickups() {
        let state = json!({
            "round": {
                "phase": "active",
                "elapsedSeconds": 42.0,
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
                    "position": { "x": 10.0, "y": 1.0, "z": 0.0 },
                    "status": "running",
                    "coinCount": 0,
                    "collectedCoins": {}
                },
                "bob": {
                    "position": { "x": 0.0, "y": 1.0, "z": 0.0 },
                    "status": "running",
                    "coinCount": 0,
                    "collectedCoins": {}
                }
            }
        });
        let mut authority = AuthorityBoundary::new(maze_rules(), state.clone());

        let finish = authority.execute("alice", Command::new("alice-finish", "finish", json!({})));
        assert!(matches!(finish.outcome, CommandOutcome::Accepted { .. }));
        assert_eq!(authority.state()["players"]["alice"]["status"], "completed");
        assert_eq!(authority.state()["players"]["bob"]["status"], "running");

        let after_finish = authority.execute(
            "alice",
            Command::new(
                "alice-after-finish",
                "collect_coin",
                json!({ "coinId": "maze-coin-01" }),
            ),
        );
        assert!(matches!(
            after_finish.outcome,
            CommandOutcome::Rejected { ref rejection, .. }
                if rejection.code == "player_run_not_active"
        ));

        let timeout = authority.execute(
            "system:clock",
            Command::new(
                "round-timeout",
                "advance_time",
                json!({ "elapsedSeconds": 180.0 }),
            ),
        );
        assert!(matches!(timeout.outcome, CommandOutcome::Accepted { .. }));
        assert_eq!(authority.state()["round"]["phase"], "timed_out");

        let after_timeout = authority.execute(
            "bob",
            Command::new(
                "bob-after-timeout",
                "collect_coin",
                json!({ "coinId": "maze-coin-01" }),
            ),
        );
        assert!(matches!(
            after_timeout.outcome,
            CommandOutcome::Rejected { ref rejection, .. }
                if rejection.code == "round_not_active"
        ));
        assert_eq!(authority.state()["players"]["bob"]["status"], "running");
        assert!(state["players"]["bob"]["collectedCoins"].is_object());
    }
}
