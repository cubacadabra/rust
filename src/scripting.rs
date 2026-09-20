use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::ui::{UiEvent, UiRuntime};
use scheduler::TaskScheduler;

mod audio;
mod api {
    include!("scripting/api.rs");
}
pub mod authority;
mod effects;
mod lua_values;
mod network;
mod scheduler;

use api::create_api;
pub(crate) use lua_values::{
    create_table, json_to_lua, lua_value_to_json, queue_has_capacity, ui_document_source,
};

#[cfg(target_arch = "wasm32")]
use luaur_rt as lua;
#[cfg(not(target_arch = "wasm32"))]
use mlua as lua;

#[allow(dead_code)]
#[derive(Default, Debug)]
pub(crate) struct ScriptState {
    pub(crate) lobby_status: String,
    pub(crate) lobby_enabled: Option<bool>,
    pub(crate) session_name: Option<String>,
    /// The package world currently presented to this script. This is kept in
    /// script state rather than captured by a Lua closure so host world
    /// transitions can update the value without rebuilding the VM.
    pub(crate) world_id: String,
    /// Package-owned world IDs accepted by `api.world:enter`.
    pub(crate) world_ids: Vec<String>,
    /// A transition requested by game code. Requests are consumed by the
    /// engine after the current callback/tick returns.
    pub(crate) world_transition: Option<String>,
    #[cfg(debug_assertions)]
    pub(crate) debug_teleport: Option<DebugTeleportRequest>,
    pub(crate) last_error: Option<String>,
    pub(crate) interactions: InteractionScriptState,
    pub(crate) network_outbox: VecDeque<String>,
    pub(crate) network_inbox: VecDeque<String>,
    pub(crate) audio_outbox: VecDeque<String>,
    pub(crate) effect_outbox: VecDeque<crate::effects::EffectCommand>,
}

#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
pub(crate) struct DebugTeleportRequest {
    pub(crate) world_id: String,
    pub(crate) position: [f32; 3],
    pub(crate) yaw: f32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct InteractionScriptState {
    pub(crate) zones: Vec<InteractionZoneState>,
    pub(crate) event_id: u32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct InteractionZoneState {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) label: String,
    pub(crate) inside: bool,
    pub(crate) nearby: bool,
    pub(crate) players: usize,
}

pub(crate) struct GameScript {
    lua: lua::Lua,
    api: lua::Table,
    on_tick: Option<lua::Function>,
    on_launch: Option<lua::Function>,
    on_ui_event: Option<lua::Function>,
    on_interaction: Option<lua::Function>,
    on_player_event: Option<lua::Function>,
    on_network_message: Option<lua::Function>,
    on_save: Option<lua::Function>,
    on_restore: Option<lua::Function>,
    state: Rc<RefCell<ScriptState>>,
    ui: Rc<RefCell<UiRuntime>>,
    scheduler: Rc<RefCell<TaskScheduler>>,
    execution_budget: Rc<RefCell<ExecutionBudget>>,
}

#[derive(Default)]
struct ExecutionBudget {
    active: bool,
    exhausted: bool,
    safepoints: u32,
}

const MAX_SCRIPT_SAFEPOINTS_PER_TICK: u32 = 100_000;

fn execute_with_budget<T>(
    execution_budget: &Rc<RefCell<ExecutionBudget>>,
    operation: impl FnOnce() -> lua::Result<T>,
) -> lua::Result<T> {
    {
        let mut budget = execution_budget.borrow_mut();
        budget.active = true;
        budget.exhausted = false;
        budget.safepoints = 0;
    }
    let result = operation();
    let exhausted = {
        let mut budget = execution_budget.borrow_mut();
        budget.active = false;
        budget.exhausted
    };
    if exhausted {
        Err(lua::Error::RuntimeError(
            "game script execution budget exceeded".to_owned(),
        ))
    } else {
        result
    }
}

impl GameScript {
    pub(crate) fn load(source: &str, ui: Rc<RefCell<UiRuntime>>) -> Result<Self, String> {
        Self::load_with_worlds(source, ui, "", Vec::new())
    }

    pub(crate) fn load_with_worlds(
        source: &str,
        ui: Rc<RefCell<UiRuntime>>,
        world_id: &str,
        world_ids: Vec<String>,
    ) -> Result<Self, String> {
        Self::load_inner(source, ui, world_id, world_ids).map_err(|error| error.to_string())
    }

    fn load_inner(
        source: &str,
        ui: Rc<RefCell<UiRuntime>>,
        world_id: &str,
        world_ids: Vec<String>,
    ) -> lua::Result<Self> {
        let lua = lua::Lua::new();
        let state = Rc::new(RefCell::new(ScriptState {
            world_id: world_id.to_owned(),
            world_ids,
            ..ScriptState::default()
        }));
        let scheduler = Rc::new(RefCell::new(TaskScheduler::default()));
        let execution_budget = Rc::new(RefCell::new(ExecutionBudget::default()));
        let interrupt_budget = Rc::clone(&execution_budget);
        lua.set_interrupt(move |_| {
            let mut budget = interrupt_budget.borrow_mut();
            if !budget.active {
                return Ok(lua::VmState::Continue);
            }
            budget.safepoints = budget.safepoints.saturating_add(1);
            if budget.safepoints >= MAX_SCRIPT_SAFEPOINTS_PER_TICK {
                budget.exhausted = true;
                Err(lua::Error::RuntimeError(
                    "game script execution budget exceeded".to_owned(),
                ))
            } else {
                Ok(lua::VmState::Continue)
            }
        });
        let api = create_api(
            &lua,
            Rc::clone(&state),
            Rc::clone(&ui),
            Rc::clone(&scheduler),
        )?;
        lua.sandbox(true)?;
        let module: lua::Table = execute_with_budget(&execution_budget, || {
            lua.load(source).set_name("game.luau").eval()
        })?;
        let on_start: Option<lua::Function> = module.get("on_start")?;
        let on_tick: Option<lua::Function> = module.get("on_tick")?;
        let on_launch: Option<lua::Function> = module.get("on_launch")?;
        let on_ui_event: Option<lua::Function> = module.get("on_ui_event")?;
        let on_interaction: Option<lua::Function> = module.get("on_interaction")?;
        let on_player_event: Option<lua::Function> = module.get("on_player_event")?;
        let on_network_message: Option<lua::Function> = module.get("on_network_message")?;
        let on_save: Option<lua::Function> = module.get("on_save")?;
        let on_restore: Option<lua::Function> = module.get("on_restore")?;

        if let Some(on_start) = on_start {
            execute_with_budget(&execution_budget, || on_start.call::<()>((api.clone(),)))?;
        }

        Ok(Self {
            lua,
            api,
            on_tick,
            on_launch,
            on_ui_event,
            on_interaction,
            on_player_event,
            on_network_message,
            on_save,
            on_restore,
            state,
            ui,
            scheduler,
            execution_budget,
        })
    }

    pub(crate) fn tick(&self, delta: f32) -> Result<(), String> {
        self.scheduler.borrow_mut().begin_tick(delta);
        let network_messages = self
            .state
            .borrow_mut()
            .network_inbox
            .drain(..)
            .collect::<Vec<_>>();
        for message in network_messages {
            self.dispatch_network_message(&message)?;
        }
        let events = self.ui.borrow_mut().take_script_events();
        for event in events {
            self.dispatch_ui_event(&event)?;
        }
        if let Some(on_tick) = &self.on_tick {
            self.execute_budgeted(|| on_tick.call::<()>((self.api.clone(), delta)))?;
        }
        self.run_tasks();
        Ok(())
    }

    fn execute_budgeted<T>(&self, operation: impl FnOnce() -> lua::Result<T>) -> Result<T, String> {
        execute_with_budget(&self.execution_budget, operation).map_err(|error| error.to_string())
    }

    fn run_tasks(&self) {
        {
            let mut budget = self.execution_budget.borrow_mut();
            budget.active = true;
            budget.exhausted = false;
            budget.safepoints = 0;
        }
        loop {
            let Some(run) = self.scheduler.borrow_mut().next() else {
                break;
            };
            let result = if let Some(value) = run.resume_value {
                run.thread.resume::<lua::MultiValue>((value,))
            } else {
                run.thread.resume::<lua::MultiValue>(())
            };
            let budget_interrupted = {
                let mut budget = self.execution_budget.borrow_mut();
                budget.active = false;
                let interrupted = budget.exhausted;
                budget.active = true;
                interrupted
            };
            if let Some(error) = self
                .scheduler
                .borrow_mut()
                .finish(run, result, budget_interrupted)
            {
                self.state.borrow_mut().last_error = Some(error);
            }
            if self.execution_budget.borrow().exhausted {
                break;
            }
        }
        self.execution_budget.borrow_mut().active = false;
    }

    fn dispatch_network_message(&self, source: &str) -> Result<(), String> {
        let Some(on_network_message) = &self.on_network_message else {
            return Ok(());
        };
        let value: serde_json::Value = serde_json::from_str(source)
            .map_err(|error| format!("network message is not valid JSON: {error}"))?;
        let value = json_to_lua(&self.lua, &value, 0).map_err(|error| error.to_string())?;
        self.execute_budgeted(|| on_network_message.call::<()>((self.api.clone(), value)))
    }

    pub(crate) fn enqueue_network_message(&self, source: &str) -> bool {
        if source.len() > crate::engine::identity::MAX_NETWORK_MESSAGE_BYTES
            || serde_json::from_str::<serde_json::Value>(source).is_err()
        {
            return false;
        }
        let mut state = self.state.borrow_mut();
        if !queue_has_capacity(&state.network_inbox, source.len()) {
            return false;
        }
        state.network_inbox.push_back(source.to_owned());
        true
    }

    pub(crate) fn take_network_message(&self) -> Option<String> {
        self.state.borrow_mut().network_outbox.pop_front()
    }

    pub(crate) fn take_audio_message(&self) -> Option<String> {
        self.state.borrow_mut().audio_outbox.pop_front()
    }

    pub(crate) fn take_effect_commands(&self) -> Vec<crate::effects::EffectCommand> {
        self.state.borrow_mut().effect_outbox.drain(..).collect()
    }

    #[cfg(debug_assertions)]
    pub(crate) fn take_debug_teleport(&self) -> Option<DebugTeleportRequest> {
        self.state.borrow_mut().debug_teleport.take()
    }

    pub(crate) fn set_world_id(&self, world_id: &str) {
        self.state.borrow_mut().world_id = world_id.to_owned();
    }

    pub(crate) fn take_world_transition(&self) -> Option<String> {
        self.state.borrow_mut().world_transition.take()
    }

    fn dispatch_ui_event(&self, event: &UiEvent) -> Result<(), String> {
        let Some(on_ui_event) = &self.on_ui_event else {
            return Ok(());
        };
        let value = create_table(&self.lua).map_err(|error| error.to_string())?;
        value
            .set("node_id", event.node_id.as_str())
            .map_err(|error| error.to_string())?;
        value
            .set("action", event.action.as_str())
            .map_err(|error| error.to_string())?;
        value
            .set("phase", event.phase.as_str())
            .map_err(|error| error.to_string())?;
        if let Some(number) = event.value {
            value
                .set("value", number)
                .map_err(|error| error.to_string())?;
        }
        if let Some(x) = event.x {
            value.set("x", x).map_err(|error| error.to_string())?;
        }
        if let Some(y) = event.y {
            value.set("y", y).map_err(|error| error.to_string())?;
        }
        self.execute_budgeted(|| on_ui_event.call::<()>((self.api.clone(), value)))
    }

    pub(crate) fn state(&self) -> Rc<RefCell<ScriptState>> {
        Rc::clone(&self.state)
    }

    /// Calls the explicit game persistence hook. The Lua VM, closures, and
    /// coroutine internals are never serialized; the hook owns the portable
    /// JSON-compatible game state contract.
    pub(crate) fn save_state(&self) -> Result<serde_json::Value, String> {
        let Some(on_save) = &self.on_save else {
            return Ok(serde_json::Value::Null);
        };
        let value = self.execute_budgeted(|| on_save.call::<lua::Value>((self.api.clone(),)))?;
        lua_value_to_json(value, 0)
    }

    pub(crate) fn pending_network_messages(&self) -> Vec<String> {
        self.state.borrow().network_inbox.iter().cloned().collect()
    }

    pub(crate) fn restore_pending_network_messages(
        &self,
        messages: &[String],
    ) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        state.network_inbox.clear();
        for message in messages {
            if message.len() > crate::engine::identity::MAX_NETWORK_MESSAGE_BYTES
                || serde_json::from_str::<serde_json::Value>(message).is_err()
                || !queue_has_capacity(&state.network_inbox, message.len())
            {
                return Err("snapshot contains an invalid or oversized network message".to_owned());
            }
            state.network_inbox.push_back(message.clone());
        }
        Ok(())
    }

    pub(crate) fn restore_state(&self, state: &serde_json::Value) -> Result<(), String> {
        let Some(on_restore) = &self.on_restore else {
            if state.is_null() {
                return Ok(());
            }
            return Err(
                "snapshot contains game state but script has no on_restore hook".to_owned(),
            );
        };
        let value = json_to_lua(&self.lua, state, 0).map_err(|error| error.to_string())?;
        self.execute_budgeted(|| on_restore.call::<()>((self.api.clone(), value)))
    }

    pub(crate) fn lobby_enabled_override(&self) -> Option<bool> {
        self.state.borrow().lobby_enabled
    }

    pub(crate) fn set_interaction_state(&self, interactions: InteractionScriptState) {
        self.state.borrow_mut().interactions = interactions;
    }

    pub(crate) fn interaction(&self, event: &crate::types::InteractionEvent) -> Result<(), String> {
        let Some(on_interaction) = &self.on_interaction else {
            return Ok(());
        };
        let value = create_table(&self.lua).map_err(|error| error.to_string())?;
        value
            .set("id", event.id.as_str())
            .map_err(|error| error.to_string())?;
        value
            .set("phase", event.phase.as_str())
            .map_err(|error| error.to_string())?;
        value
            .set("players", event.players)
            .map_err(|error| error.to_string())?;
        value
            .set(
                "position",
                self.lua
                    .create_sequence_from(event.position.iter().copied())
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        self.execute_budgeted(|| on_interaction.call::<()>((self.api.clone(), value)))
    }

    pub(crate) fn player_event(&self, event: &crate::types::PlayerEvent) -> Result<(), String> {
        let Some(on_player_event) = &self.on_player_event else {
            return Ok(());
        };
        let value = create_table(&self.lua).map_err(|error| error.to_string())?;
        value
            .set("type", "player")
            .map_err(|error| error.to_string())?;
        match event {
            crate::types::PlayerEvent::Spawn {
                health,
                max_health,
                deaths,
            } => {
                value
                    .set("kind", "spawn")
                    .map_err(|error| error.to_string())?;
                value
                    .set("health", *health)
                    .map_err(|error| error.to_string())?;
                value
                    .set("maxHealth", *max_health)
                    .map_err(|error| error.to_string())?;
                value
                    .set("deaths", *deaths)
                    .map_err(|error| error.to_string())?;
            }
            crate::types::PlayerEvent::Checkpoint { id, position } => {
                value
                    .set("kind", "checkpoint")
                    .map_err(|error| error.to_string())?;
                value
                    .set("id", id.as_str())
                    .map_err(|error| error.to_string())?;
                value
                    .set(
                        "position",
                        self.lua
                            .create_sequence_from(position.iter().copied())
                            .map_err(|error| error.to_string())?,
                    )
                    .map_err(|error| error.to_string())?;
            }
            crate::types::PlayerEvent::Death {
                cause,
                checkpoint,
                deaths,
                health,
                max_health,
            } => {
                value
                    .set("kind", "death")
                    .map_err(|error| error.to_string())?;
                value
                    .set("cause", cause.as_str())
                    .map_err(|error| error.to_string())?;
                value
                    .set("checkpoint", checkpoint.as_str())
                    .map_err(|error| error.to_string())?;
                value
                    .set("deaths", *deaths)
                    .map_err(|error| error.to_string())?;
                value
                    .set("health", *health)
                    .map_err(|error| error.to_string())?;
                value
                    .set("maxHealth", *max_health)
                    .map_err(|error| error.to_string())?;
            }
            crate::types::PlayerEvent::Respawn {
                checkpoint,
                deaths,
                health,
                max_health,
            } => {
                value
                    .set("kind", "respawn")
                    .map_err(|error| error.to_string())?;
                value
                    .set("checkpoint", checkpoint.as_str())
                    .map_err(|error| error.to_string())?;
                value
                    .set("deaths", *deaths)
                    .map_err(|error| error.to_string())?;
                value
                    .set("health", *health)
                    .map_err(|error| error.to_string())?;
                value
                    .set("maxHealth", *max_health)
                    .map_err(|error| error.to_string())?;
            }
            crate::types::PlayerEvent::Damage {
                source,
                amount,
                health,
                max_health,
            } => {
                value
                    .set("kind", "damage")
                    .map_err(|error| error.to_string())?;
                value
                    .set("source", source.as_str())
                    .map_err(|error| error.to_string())?;
                value
                    .set("amount", *amount)
                    .map_err(|error| error.to_string())?;
                value
                    .set("health", *health)
                    .map_err(|error| error.to_string())?;
                value
                    .set("maxHealth", *max_health)
                    .map_err(|error| error.to_string())?;
            }
            crate::types::PlayerEvent::Heal {
                source,
                amount,
                health,
                max_health,
            } => {
                value
                    .set("kind", "heal")
                    .map_err(|error| error.to_string())?;
                value
                    .set("source", source.as_str())
                    .map_err(|error| error.to_string())?;
                value
                    .set("amount", *amount)
                    .map_err(|error| error.to_string())?;
                value
                    .set("health", *health)
                    .map_err(|error| error.to_string())?;
                value
                    .set("maxHealth", *max_health)
                    .map_err(|error| error.to_string())?;
            }
        }
        self.execute_budgeted(|| on_player_event.call::<()>((self.api.clone(), value)))
    }

    #[allow(dead_code)]
    pub(crate) fn launch(&self, pad_id: &str, player_ids: &[u32]) -> Result<(), String> {
        let Some(on_launch) = &self.on_launch else {
            return Ok(());
        };
        let launch = create_table(&self.lua).map_err(|error| error.to_string())?;
        launch
            .set("pad_id", pad_id)
            .map_err(|error| error.to_string())?;
        launch
            .set(
                "player_ids",
                self.lua
                    .create_sequence_from(player_ids.iter().copied())
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        self.execute_budgeted(|| on_launch.call::<()>((self.api.clone(), launch)))
    }
}

#[cfg(test)]
#[path = "scripting_tests.rs"]
mod scripting_tests;
