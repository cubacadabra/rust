//! Generic command authority primitives.
//!
//! This module deliberately knows nothing about a game's rules. It provides
//! the boundary a trusted world implementation can use:
//!
//! ```text
//! Command -> validate -> simulate -> Event/State
//! ```
//!
//! A game supplies a [`CommandHandler`] that interprets the command name and
//! payload. The boundary owns envelope validation, event sequencing, and the
//! atomic state transition. It is usable by a headless host or a future
//! server-side WebAssembly worker; it is not connected to the current
//! Cloudflare WebSocket protocol yet.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const AUTHORITY_PROTOCOL_VERSION: u8 = 1;

/// A client request with no client-authored resulting state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub request_id: String,
    pub actor_id: String,
    pub name: String,
    #[serde(default)]
    pub payload: Value,
}

impl Command {
    pub fn new(
        request_id: impl Into<String>,
        actor_id: impl Into<String>,
        name: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            actor_id: actor_id.into(),
            name: name.into(),
            payload,
        }
    }
}

/// An event description returned by a game simulation before the boundary
/// assigns its authoritative sequence number.
#[derive(Clone, Debug, PartialEq)]
pub struct EventSpec {
    pub name: String,
    pub payload: Value,
}

impl EventSpec {
    pub fn new(name: impl Into<String>, payload: Value) -> Self {
        Self {
            name: name.into(),
            payload,
        }
    }
}

/// An event emitted by an accepted command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub version: u8,
    pub sequence: u64,
    pub request_id: String,
    pub actor_id: String,
    pub name: String,
    pub payload: Value,
}

/// The result of a game-specific simulation, before event sequence numbers
/// and the public outcome are assembled by [`AuthorityBoundary`].
#[derive(Clone, Debug, PartialEq)]
pub struct Simulation {
    pub state: Value,
    pub events: Vec<EventSpec>,
}

impl Simulation {
    pub fn new(state: Value, events: Vec<EventSpec>) -> Self {
        Self { state, events }
    }
}

/// A stable, machine-readable reason for rejecting a command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    pub code: String,
}

impl Rejection {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

/// The generic contract a game-owned ruleset implements.
pub trait CommandHandler {
    /// Check a command against the current authoritative state and trusted
    /// world context. This method must not mutate state.
    fn validate(&self, state: &Value, command: &Command) -> Result<(), Rejection>;

    /// Return the complete next state and emitted event descriptions. The
    /// boundary only commits the returned state after this succeeds.
    fn simulate(&self, state: &Value, command: &Command) -> Result<Simulation, Rejection>;
}

/// The generic authority boundary for one authoritative world/state stream.
///
/// This is intentionally small. It does not persist state, authenticate
/// actors, run a Lua VM, or open sockets. Those are host concerns around this
/// boundary.
pub struct AuthorityBoundary<H> {
    handler: H,
    state: Value,
    next_event_sequence: u64,
}

impl<H> AuthorityBoundary<H>
where
    H: CommandHandler,
{
    pub fn new(handler: H, state: Value) -> Self {
        Self {
            handler,
            state,
            next_event_sequence: 0,
        }
    }

    pub fn state(&self) -> &Value {
        &self.state
    }

    pub fn next_event_sequence(&self) -> u64 {
        self.next_event_sequence.saturating_add(1)
    }

    pub fn execute(&mut self, command: Command) -> CommandOutcome {
        if command.request_id.trim().is_empty()
            || command.actor_id.trim().is_empty()
            || command.name.trim().is_empty()
        {
            return CommandOutcome::Rejected {
                request_id: command.request_id,
                rejection: Rejection::new("invalid_command_envelope"),
            };
        }

        if let Err(rejection) = self.handler.validate(&self.state, &command) {
            return CommandOutcome::Rejected {
                request_id: command.request_id,
                rejection,
            };
        }

        let simulation = match self.handler.simulate(&self.state, &command) {
            Ok(simulation) => simulation,
            Err(rejection) => {
                return CommandOutcome::Rejected {
                    request_id: command.request_id,
                    rejection,
                };
            }
        };

        let mut events = Vec::with_capacity(simulation.events.len());
        for event in simulation.events {
            self.next_event_sequence = self.next_event_sequence.saturating_add(1);
            events.push(Event {
                version: AUTHORITY_PROTOCOL_VERSION,
                sequence: self.next_event_sequence,
                request_id: command.request_id.clone(),
                actor_id: command.actor_id.clone(),
                name: event.name,
                payload: event.payload,
            });
        }
        self.state = simulation.state;

        CommandOutcome::Accepted {
            request_id: command.request_id,
            state: self.state.clone(),
            events,
        }
    }
}

/// The only state transition a client may observe from the authority
/// boundary: accepted commands produce the new state and events; rejected
/// commands produce neither.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum CommandOutcome {
    Accepted {
        request_id: String,
        state: Value,
        events: Vec<Event>,
    },
    Rejected {
        request_id: String,
        rejection: Rejection,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A tiny stand-in for Signal Run's eventual server rules. The game owns
    /// the state shape and the meaning of "interact"; the boundary does not.
    struct GateRules;

    impl CommandHandler for GateRules {
        fn validate(&self, state: &Value, command: &Command) -> Result<(), Rejection> {
            if command.name != "interact" {
                return Err(Rejection::new("unsupported_command"));
            }
            let target = command
                .payload
                .get("target")
                .and_then(Value::as_str)
                .ok_or_else(|| Rejection::new("missing_target"))?;
            let actor = state
                .get("actors")
                .and_then(|actors| actors.get(&command.actor_id))
                .ok_or_else(|| Rejection::new("actor_not_in_world"))?;
            let zone = state
                .get("zones")
                .and_then(|zones| zones.get(target))
                .ok_or_else(|| Rejection::new("unknown_interaction"))?;
            if zone.get("captured").and_then(Value::as_bool) == Some(true) {
                return Err(Rejection::new("already_captured"));
            }

            let distance = (actor["x"].as_f64().unwrap_or_default()
                - zone["x"].as_f64().unwrap_or_default())
            .hypot(
                actor["z"].as_f64().unwrap_or_default() - zone["z"].as_f64().unwrap_or_default(),
            );
            if distance > zone["radius"].as_f64().unwrap_or_default() {
                return Err(Rejection::new("outside_interaction_radius"));
            }
            Ok(())
        }

        fn simulate(&self, state: &Value, command: &Command) -> Result<Simulation, Rejection> {
            self.validate(state, command)?;
            let target = command.payload["target"].as_str().unwrap();
            let mut next = state.clone();
            next["zones"][target]["captured"] = json!(true);
            Ok(Simulation::new(
                next,
                vec![EventSpec::new(
                    "interaction_accepted",
                    json!({ "target": target }),
                )],
            ))
        }
    }

    fn boundary(actor_x: f64) -> AuthorityBoundary<GateRules> {
        AuthorityBoundary::new(
            GateRules,
            json!({
                "actors": { "player-1": { "x": actor_x, "z": 0.0 } },
                "zones": {
                    "gate-a": { "x": 1.0, "z": 1.0, "radius": 2.0, "captured": false }
                }
            }),
        )
    }

    #[test]
    fn accepts_an_in_range_interaction_and_emits_event_and_state() {
        let mut boundary = boundary(0.0);
        let before = boundary.state().clone();
        let outcome = boundary.execute(Command::new(
            "request-1",
            "player-1",
            "interact",
            json!({ "target": "gate-a" }),
        ));

        assert_eq!(
            outcome,
            CommandOutcome::Accepted {
                request_id: "request-1".to_owned(),
                state: json!({
                    "actors": { "player-1": { "x": 0.0, "z": 0.0 } },
                    "zones": {
                        "gate-a": { "x": 1.0, "z": 1.0, "radius": 2.0, "captured": true }
                    }
                }),
                events: vec![Event {
                    version: 1,
                    sequence: 1,
                    request_id: "request-1".to_owned(),
                    actor_id: "player-1".to_owned(),
                    name: "interaction_accepted".to_owned(),
                    payload: json!({ "target": "gate-a" }),
                }],
            }
        );
        assert_ne!(before, boundary.state().clone());
        assert_eq!(boundary.next_event_sequence(), 2);
    }

    #[test]
    fn rejects_an_out_of_range_interaction_without_committing_state_or_events() {
        let mut boundary = boundary(10.0);
        let before = boundary.state().clone();
        let outcome = boundary.execute(Command::new(
            "request-2",
            "player-1",
            "interact",
            json!({ "target": "gate-a" }),
        ));

        assert_eq!(
            outcome,
            CommandOutcome::Rejected {
                request_id: "request-2".to_owned(),
                rejection: Rejection::new("outside_interaction_radius"),
            }
        );
        assert_eq!(boundary.state(), &before);
        assert_eq!(boundary.next_event_sequence(), 1);
    }
}
