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

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const AUTHORITY_PROTOCOL_VERSION: u8 = 1;
pub const MAX_AUTHORITY_COMMAND_BYTES: usize = 16 * 1024;
pub const MAX_AUTHORITY_STATE_BYTES: usize = 1024 * 1024;
pub const MAX_AUTHORITY_RESULT_BYTES: usize = 64 * 1024;
pub const MAX_AUTHORITY_EVENTS_PER_COMMAND: usize = 64;
const MAX_RECENT_COMMANDS: usize = 32;
const AUTHORITY_SNAPSHOT_VERSION: u8 = 1;
const MAX_AUTHORITY_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;

/// An untrusted client request. Actor identity is deliberately absent: the
/// trusted transport must bind this intent to the identity on its connection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub request_id: String,
    pub name: String,
    #[serde(default)]
    pub payload: Value,
}

impl Command {
    pub fn new(request_id: impl Into<String>, name: impl Into<String>, payload: Value) -> Self {
        Self {
            request_id: request_id.into(),
            name: name.into(),
            payload,
        }
    }
}

/// A command after the trusted host has attached the authenticated actor.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthenticatedCommand {
    pub request_id: String,
    pub actor_id: String,
    pub name: String,
    pub payload: Value,
}

/// A command result plus retry metadata for the transport adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandExecution {
    pub outcome: CommandOutcome,
    /// Replayed requests must not be broadcast as a second state transition.
    pub replayed: bool,
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
    /// world context. Actor identity comes from the trusted host, not payload.
    /// This method must not mutate state.
    fn validate(&self, state: &Value, command: &AuthenticatedCommand) -> Result<(), Rejection>;

    /// Return the complete next state and emitted event descriptions. The
    /// boundary only commits the returned state after this succeeds.
    fn simulate(
        &self,
        state: &Value,
        command: &AuthenticatedCommand,
    ) -> Result<Simulation, Rejection>;
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
    recent_commands: VecDeque<CachedCommand>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedCommand {
    actor_id: String,
    command: Command,
    result: CachedResult,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum CachedResult {
    Accepted { events: Vec<Event> },
    Rejected { rejection: Rejection },
}

/// Durable checkpoint for one trusted authority stream. Persist this whole
/// value atomically; keeping the request receipts beside state prevents a
/// retry after restart from reapplying an accepted command.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoritySnapshot {
    version: u8,
    state: Value,
    next_event_sequence: u64,
    recent_commands: Vec<CachedCommand>,
}

impl AuthoritySnapshot {
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        let source = serde_json::to_string(self).map_err(|error| error.to_string())?;
        if source.len() > MAX_AUTHORITY_SNAPSHOT_BYTES {
            return Err("authority snapshot exceeds the 4 MiB limit".to_owned());
        }
        Ok(source)
    }

    pub fn from_json(source: &str) -> Result<Self, String> {
        if source.len() > MAX_AUTHORITY_SNAPSHOT_BYTES {
            return Err("authority snapshot exceeds the 4 MiB limit".to_owned());
        }
        let snapshot: Self = serde_json::from_str(source)
            .map_err(|error| format!("invalid authority snapshot: {error}"))?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    fn validate(&self) -> Result<(), String> {
        if self.version != AUTHORITY_SNAPSHOT_VERSION {
            return Err("unsupported authority snapshot version".to_owned());
        }
        if serde_json::to_vec(&self.state)
            .map_or(true, |encoded| encoded.len() > MAX_AUTHORITY_STATE_BYTES)
        {
            return Err("authority snapshot state exceeds the 1 MiB limit".to_owned());
        }
        if self.recent_commands.len() > MAX_RECENT_COMMANDS {
            return Err("authority snapshot has too many command receipts".to_owned());
        }

        let mut request_ids = std::collections::BTreeSet::new();
        let mut previous_event_sequence = 0;
        for cached in &self.recent_commands {
            if !valid_envelope(&cached.actor_id, &cached.command)
                || !request_ids
                    .insert((cached.actor_id.as_str(), cached.command.request_id.as_str()))
            {
                return Err("authority snapshot contains an invalid command receipt".to_owned());
            }
            match &cached.result {
                CachedResult::Accepted { events } => {
                    if events.len() > MAX_AUTHORITY_EVENTS_PER_COMMAND
                        || serde_json::to_vec(events)
                            .map_or(true, |encoded| encoded.len() > MAX_AUTHORITY_RESULT_BYTES)
                    {
                        return Err(
                            "authority snapshot contains oversized command events".to_owned()
                        );
                    }
                    for event in events {
                        if event.version != AUTHORITY_PROTOCOL_VERSION
                            || event.request_id != cached.command.request_id
                            || event.actor_id != cached.actor_id
                            || event.sequence <= previous_event_sequence
                            || event.sequence > self.next_event_sequence
                            || event.name.trim().is_empty()
                            || event.name.len() > 96
                        {
                            return Err(
                                "authority snapshot contains an invalid command event".to_owned()
                            );
                        }
                        previous_event_sequence = event.sequence;
                    }
                }
                CachedResult::Rejected { rejection } => {
                    if rejection.code.trim().is_empty() || rejection.code.len() > 96 {
                        return Err("authority snapshot contains an invalid rejection".to_owned());
                    }
                }
            }
        }
        Ok(())
    }
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
            recent_commands: VecDeque::new(),
        }
    }

    pub fn state(&self) -> &Value {
        &self.state
    }

    pub fn next_event_sequence(&self) -> u64 {
        self.next_event_sequence.saturating_add(1)
    }

    pub fn snapshot(&self) -> AuthoritySnapshot {
        AuthoritySnapshot {
            version: AUTHORITY_SNAPSHOT_VERSION,
            state: self.state.clone(),
            next_event_sequence: self.next_event_sequence,
            recent_commands: self.recent_commands.iter().cloned().collect(),
        }
    }

    pub fn restore(handler: H, snapshot: AuthoritySnapshot) -> Result<Self, String> {
        snapshot.validate()?;
        Ok(Self {
            handler,
            state: snapshot.state,
            next_event_sequence: snapshot.next_event_sequence,
            recent_commands: snapshot.recent_commands.into(),
        })
    }

    pub fn snapshot_json(&self) -> Result<String, String> {
        self.snapshot().to_json()
    }

    pub fn restore_json(handler: H, source: &str) -> Result<Self, String> {
        Self::restore(handler, AuthoritySnapshot::from_json(source)?)
    }

    /// Executes an untrusted intent as the identity authenticated by the
    /// trusted host. Never pass an actor ID supplied by the client here.
    pub fn execute(&mut self, actor_id: &str, command: Command) -> CommandExecution {
        let authenticated = AuthenticatedCommand {
            request_id: command.request_id.clone(),
            actor_id: actor_id.to_owned(),
            name: command.name.clone(),
            payload: command.payload.clone(),
        };

        if !valid_envelope(actor_id, &command) {
            return rejected_execution(command.request_id, "invalid_command_envelope", false);
        }

        if let Some(cached) = self.recent_commands.iter().find(|cached| {
            cached.actor_id == actor_id && cached.command.request_id == command.request_id
        }) {
            if cached.command != command {
                return rejected_execution(command.request_id, "request_id_reused", false);
            }
            let outcome = match &cached.result {
                CachedResult::Accepted { events } => CommandOutcome::Accepted {
                    request_id: command.request_id,
                    state: self.state.clone(),
                    events: events.clone(),
                },
                CachedResult::Rejected { rejection } => CommandOutcome::Rejected {
                    request_id: command.request_id,
                    rejection: rejection.clone(),
                },
            };
            return CommandExecution {
                outcome,
                replayed: true,
            };
        }

        let result = self.execute_once(&authenticated);
        let cached_result = match &result {
            CommandOutcome::Accepted { events, .. } => CachedResult::Accepted {
                events: events.clone(),
            },
            CommandOutcome::Rejected { rejection, .. } => CachedResult::Rejected {
                rejection: rejection.clone(),
            },
        };
        self.recent_commands.push_back(CachedCommand {
            actor_id: actor_id.to_owned(),
            command,
            result: cached_result,
        });
        if self.recent_commands.len() > MAX_RECENT_COMMANDS {
            self.recent_commands.pop_front();
        }

        CommandExecution {
            outcome: result,
            replayed: false,
        }
    }

    fn execute_once(&mut self, command: &AuthenticatedCommand) -> CommandOutcome {
        if let Err(rejection) = self.handler.validate(&self.state, command) {
            return CommandOutcome::Rejected {
                request_id: command.request_id.clone(),
                rejection,
            };
        }

        let simulation = match self.handler.simulate(&self.state, command) {
            Ok(simulation) => simulation,
            Err(rejection) => {
                return CommandOutcome::Rejected {
                    request_id: command.request_id.clone(),
                    rejection,
                };
            }
        };

        if simulation.events.len() > MAX_AUTHORITY_EVENTS_PER_COMMAND
            || serde_json::to_vec(&simulation.state)
                .map_or(true, |encoded| encoded.len() > MAX_AUTHORITY_STATE_BYTES)
        {
            return CommandOutcome::Rejected {
                request_id: command.request_id.clone(),
                rejection: Rejection::new("authority_result_too_large"),
            };
        }

        let mut events = Vec::with_capacity(simulation.events.len());
        let mut next_sequence = self.next_event_sequence;
        for event in simulation.events {
            let Some(sequence) = next_sequence.checked_add(1) else {
                return CommandOutcome::Rejected {
                    request_id: command.request_id.clone(),
                    rejection: Rejection::new("authority_sequence_exhausted"),
                };
            };
            next_sequence = sequence;
            events.push(Event {
                version: AUTHORITY_PROTOCOL_VERSION,
                sequence,
                request_id: command.request_id.clone(),
                actor_id: command.actor_id.clone(),
                name: event.name,
                payload: event.payload,
            });
        }
        if events
            .iter()
            .any(|event| event.name.trim().is_empty() || event.name.len() > 96)
            || serde_json::to_vec(&events)
                .map_or(true, |encoded| encoded.len() > MAX_AUTHORITY_RESULT_BYTES)
        {
            return CommandOutcome::Rejected {
                request_id: command.request_id.clone(),
                rejection: Rejection::new("authority_result_too_large"),
            };
        }

        self.next_event_sequence = next_sequence;
        self.state = simulation.state;

        CommandOutcome::Accepted {
            request_id: command.request_id.clone(),
            state: self.state.clone(),
            events,
        }
    }
}

fn valid_envelope(actor_id: &str, command: &Command) -> bool {
    let request_id_bytes = command.request_id.len();
    let actor_id_bytes = actor_id.len();
    let name_bytes = command.name.len();
    if !(1..=128).contains(&request_id_bytes)
        || command.request_id.trim().is_empty()
        || !(1..=128).contains(&actor_id_bytes)
        || actor_id.trim().is_empty()
        || !(1..=96).contains(&name_bytes)
        || command.name.trim().is_empty()
    {
        return false;
    }
    serde_json::to_vec(command).map_or(false, |encoded| {
        encoded.len() <= MAX_AUTHORITY_COMMAND_BYTES
    })
}

fn rejected_execution(request_id: String, code: &str, replayed: bool) -> CommandExecution {
    CommandExecution {
        outcome: CommandOutcome::Rejected {
            request_id,
            rejection: Rejection::new(code),
        },
        replayed,
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
        fn validate(&self, state: &Value, command: &AuthenticatedCommand) -> Result<(), Rejection> {
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

        fn simulate(
            &self,
            state: &Value,
            command: &AuthenticatedCommand,
        ) -> Result<Simulation, Rejection> {
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
        let execution = boundary.execute(
            "player-1",
            Command::new("request-1", "interact", json!({ "target": "gate-a" })),
        );
        assert!(!execution.replayed);
        let outcome = execution.outcome;

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
        let outcome = boundary
            .execute(
                "player-1",
                Command::new("request-2", "interact", json!({ "target": "gate-a" })),
            )
            .outcome;

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

    #[test]
    fn binds_actor_to_trusted_identity_and_deduplicates_retries() {
        let mut boundary = boundary(0.0);
        let command = Command::new(
            "request-3",
            "interact",
            json!({ "target": "gate-a", "actorId": "forged-player" }),
        );

        let first = boundary.execute("player-1", command.clone());
        assert!(!first.replayed);
        let CommandOutcome::Accepted { events, .. } = first.outcome else {
            panic!("the trusted actor should be able to interact");
        };
        assert_eq!(events[0].actor_id, "player-1");

        let retry = boundary.execute("player-1", command);
        assert!(retry.replayed);
        let CommandOutcome::Accepted {
            events: retried_events,
            ..
        } = retry.outcome
        else {
            panic!("the retry should return the original accepted result");
        };
        assert_eq!(retried_events, events);
        assert_eq!(boundary.next_event_sequence(), 2);
    }

    #[test]
    fn reusing_a_request_id_for_a_different_command_is_rejected() {
        let mut boundary = boundary(0.0);
        let first = Command::new("request-4", "interact", json!({ "target": "gate-a" }));
        assert!(matches!(
            boundary.execute("player-1", first).outcome,
            CommandOutcome::Accepted { .. }
        ));

        let before = boundary.state().clone();
        let reused = Command::new(
            "request-4",
            "interact",
            json!({ "target": "another-target" }),
        );
        assert_eq!(
            boundary.execute("player-1", reused),
            rejected_execution("request-4".to_owned(), "request_id_reused", false)
        );
        assert_eq!(boundary.state(), &before);
        assert_eq!(boundary.next_event_sequence(), 2);
    }

    #[test]
    fn snapshot_restores_round_state_and_retry_receipts_together() {
        let mut boundary = boundary(0.0);
        let command = Command::new("request-5", "interact", json!({ "target": "gate-a" }));
        assert!(matches!(
            boundary.execute("player-1", command.clone()).outcome,
            CommandOutcome::Accepted { .. }
        ));

        let snapshot = boundary.snapshot_json().expect("snapshot should encode");
        let mut restored =
            AuthorityBoundary::restore_json(GateRules, &snapshot).expect("snapshot should restore");
        assert_eq!(restored.state(), boundary.state());

        let retry = restored.execute("player-1", command);
        assert!(retry.replayed);
        assert!(matches!(retry.outcome, CommandOutcome::Accepted { .. }));
        assert_eq!(restored.next_event_sequence(), 2);
    }

    #[test]
    fn snapshot_restore_rejects_unknown_versions() {
        let mut source = boundary(0.0)
            .snapshot_json()
            .expect("snapshot should encode");
        let mut value: Value = serde_json::from_str(&source).expect("snapshot should be JSON");
        value["version"] = json!(AUTHORITY_SNAPSHOT_VERSION + 1);
        source = serde_json::to_string(&value).expect("snapshot should encode");

        assert!(AuthorityBoundary::restore_json(GateRules, &source).is_err());
    }
}
