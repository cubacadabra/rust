use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::ui::{UiEvent, UiRuntime};

mod audio;
mod effects;
mod network;

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
    pub(crate) last_error: Option<String>,
    pub(crate) interactions: InteractionScriptState,
    pub(crate) network_outbox: VecDeque<String>,
    pub(crate) network_inbox: VecDeque<String>,
    pub(crate) audio_outbox: VecDeque<String>,
    pub(crate) effect_outbox: VecDeque<crate::effects::EffectCommand>,
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
    state: Rc<RefCell<ScriptState>>,
    ui: Rc<RefCell<UiRuntime>>,
}

impl GameScript {
    pub(crate) fn load(source: &str, ui: Rc<RefCell<UiRuntime>>) -> Result<Self, String> {
        Self::load_inner(source, ui).map_err(|error| error.to_string())
    }

    fn load_inner(source: &str, ui: Rc<RefCell<UiRuntime>>) -> lua::Result<Self> {
        let lua = lua::Lua::new();
        lua.sandbox(true)?;
        let state = Rc::new(RefCell::new(ScriptState::default()));
        let api = create_api(&lua, Rc::clone(&state), Rc::clone(&ui))?;
        let module: lua::Table = lua.load(source).set_name("game.luau").eval()?;
        let on_start: Option<lua::Function> = module.get("on_start")?;
        let on_tick: Option<lua::Function> = module.get("on_tick")?;
        let on_launch: Option<lua::Function> = module.get("on_launch")?;
        let on_ui_event: Option<lua::Function> = module.get("on_ui_event")?;
        let on_interaction: Option<lua::Function> = module.get("on_interaction")?;
        let on_player_event: Option<lua::Function> = module.get("on_player_event")?;
        let on_network_message: Option<lua::Function> = module.get("on_network_message")?;

        if let Some(on_start) = on_start {
            on_start.call::<()>((api.clone(),))?;
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
            state,
            ui,
        })
    }

    pub(crate) fn tick(&self, delta: f32) -> Result<(), String> {
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
            on_tick
                .call::<()>((self.api.clone(), delta))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn dispatch_network_message(&self, source: &str) -> Result<(), String> {
        let Some(on_network_message) = &self.on_network_message else {
            return Ok(());
        };
        let value: serde_json::Value = serde_json::from_str(source)
            .map_err(|error| format!("network message is not valid JSON: {error}"))?;
        let value = json_to_lua(&self.lua, &value, 0).map_err(|error| error.to_string())?;
        on_network_message
            .call::<()>((self.api.clone(), value))
            .map_err(|error| error.to_string())
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
        on_ui_event
            .call::<()>((self.api.clone(), value))
            .map_err(|error| error.to_string())
    }

    pub(crate) fn state(&self) -> Rc<RefCell<ScriptState>> {
        Rc::clone(&self.state)
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
        on_interaction
            .call::<()>((self.api.clone(), value))
            .map_err(|error| error.to_string())
    }

    pub(crate) fn player_event(&self, event: &crate::types::PlayerEvent) -> Result<(), String> {
        let Some(on_player_event) = &self.on_player_event else {
            return Ok(());
        };
        let value = create_table(&self.lua).map_err(|error| error.to_string())?;
        match event {
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
            }
            crate::types::PlayerEvent::Respawn { checkpoint, deaths } => {
                value
                    .set("kind", "respawn")
                    .map_err(|error| error.to_string())?;
                value
                    .set("checkpoint", checkpoint.as_str())
                    .map_err(|error| error.to_string())?;
                value
                    .set("deaths", *deaths)
                    .map_err(|error| error.to_string())?;
            }
        }
        on_player_event
            .call::<()>((self.api.clone(), value))
            .map_err(|error| error.to_string())
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
        on_launch
            .call::<()>((self.api.clone(), launch))
            .map_err(|error| error.to_string())
    }
}

fn create_api(
    lua: &lua::Lua,
    state: Rc<RefCell<ScriptState>>,
    ui_runtime: Rc<RefCell<UiRuntime>>,
) -> lua::Result<lua::Table> {
    let api = create_table(lua)?;

    let lobby = create_table(lua)?;
    let lobby_state = Rc::clone(&state);
    let lobby_enabled_state = Rc::clone(&state);
    lobby.set(
        "set_enabled",
        lua.create_function(move |_, (_lobby, enabled): (lua::Table, bool)| {
            lobby_enabled_state.borrow_mut().lobby_enabled = Some(enabled);
            Ok(())
        })?,
    )?;
    lobby.set(
        "set_status",
        lua.create_function(move |_, (_lobby, status): (lua::Table, String)| {
            lobby_state.borrow_mut().lobby_status = status;
            Ok(())
        })?,
    )?;
    api.set("lobby", lobby)?;

    let session = create_table(lua)?;
    let session_state = Rc::clone(&state);
    session.set(
        "start",
        lua.create_function(
            move |_, (_session, name, _options): (lua::Table, String, lua::Table)| {
                session_state.borrow_mut().session_name = Some(name);
                Ok(())
            },
        )?,
    )?;
    api.set("session", session)?;

    let ui = create_table(lua)?;
    let document_runtime = Rc::clone(&ui_runtime);
    ui.set(
        "set_document",
        lua.create_function(move |_, (_ui, source): (lua::Table, lua::Value)| {
            let source = ui_document_source(source).map_err(lua::Error::runtime)?;
            document_runtime
                .borrow_mut()
                .set_document_json(&source)
                .map_err(lua::Error::runtime)?;
            Ok(())
        })?,
    )?;
    let clear_runtime = Rc::clone(&ui_runtime);
    ui.set(
        "clear",
        lua.create_function(move |_, _ui: lua::Table| {
            clear_runtime.borrow_mut().clear();
            Ok(())
        })?,
    )?;
    let text_runtime = Rc::clone(&ui_runtime);
    ui.set(
        "set_text",
        lua.create_function(move |_, (_ui, id, text): (lua::Table, String, String)| {
            Ok(text_runtime.borrow_mut().set_text(&id, &text))
        })?,
    )?;
    let value_runtime = Rc::clone(&ui_runtime);
    ui.set(
        "set_value",
        lua.create_function(move |_, (_ui, id, value): (lua::Table, String, f32)| {
            Ok(value_runtime.borrow_mut().set_value(&id, value))
        })?,
    )?;
    let checked_runtime = Rc::clone(&ui_runtime);
    ui.set(
        "set_checked",
        lua.create_function(move |_, (_ui, id, checked): (lua::Table, String, bool)| {
            Ok(checked_runtime.borrow_mut().set_checked(&id, checked))
        })?,
    )?;
    let visible_runtime = ui_runtime;
    ui.set(
        "set_visible",
        lua.create_function(move |_, (_ui, id, visible): (lua::Table, String, bool)| {
            Ok(visible_runtime.borrow_mut().set_visible(&id, visible))
        })?,
    )?;
    api.set("ui", ui)?;

    let interactions = create_table(lua)?;
    let interactions_state = Rc::clone(&state);
    interactions.set(
        "get_state",
        lua.create_function(move |lua, _interactions: lua::Table| {
            let state = interactions_state.borrow();
            let result = create_table(lua)?;
            result.set("event_id", state.interactions.event_id)?;
            let zones = create_table(lua)?;
            for zone in &state.interactions.zones {
                let value = create_table(lua)?;
                value.set("id", zone.id.as_str())?;
                value.set("kind", zone.kind.as_str())?;
                value.set("label", zone.label.as_str())?;
                value.set("inside", zone.inside)?;
                value.set("nearby", zone.nearby)?;
                value.set("players", zone.players)?;
                zones.set(zone.id.as_str(), value)?;
            }
            result.set("zones", zones)?;
            Ok(result)
        })?,
    )?;
    api.set("interactions", interactions)?;

    network::install(lua, &api, Rc::clone(&state))?;

    audio::install(lua, &api, Rc::clone(&state))?;

    effects::install(lua, &api, Rc::clone(&state))?;

    Ok(api)
}

const MAX_SCRIPT_QUEUE_MESSAGES: usize = 64;
const MAX_SCRIPT_QUEUE_BYTES: usize = 256 * 1024;

fn queue_has_capacity(queue: &VecDeque<String>, additional_bytes: usize) -> bool {
    queue.len() < MAX_SCRIPT_QUEUE_MESSAGES
        && queue.iter().map(String::len).sum::<usize>() + additional_bytes <= MAX_SCRIPT_QUEUE_BYTES
}

fn ui_document_source(value: lua::Value) -> Result<String, String> {
    match value {
        lua::Value::String(value) => Ok(value.to_string_lossy()),
        lua::Value::Table(_) => {
            serde_json::to_string(&lua_value_to_json(value, 0)?).map_err(|error| error.to_string())
        }
        value => Err(format!(
            "ui:set_document expects a table or JSON string, received {}",
            value.type_name()
        )),
    }
}

fn lua_value_to_json(value: lua::Value, depth: usize) -> Result<serde_json::Value, String> {
    if depth > 32 {
        return Err("UI document table nesting exceeds 32 levels".to_owned());
    }
    match value {
        lua::Value::Nil => Ok(serde_json::Value::Null),
        lua::Value::Boolean(value) => Ok(serde_json::Value::Bool(value)),
        lua::Value::Integer(value) => Ok(serde_json::Value::Number(value.into())),
        lua::Value::Number(value) => serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| "UI document numbers must be finite".to_owned()),
        lua::Value::String(value) => Ok(serde_json::Value::String(value.to_string_lossy())),
        lua::Value::Table(table) if table.raw_len() > 0 => {
            let mut values = Vec::with_capacity(table.raw_len());
            for index in 1..=table.raw_len() {
                let value = table
                    .get::<lua::Value>(index)
                    .map_err(|error| error.to_string())?;
                values.push(lua_value_to_json(value, depth + 1)?);
            }
            Ok(serde_json::Value::Array(values))
        }
        lua::Value::Table(table) => {
            let mut values = serde_json::Map::new();
            for pair in table.pairs::<lua::Value, lua::Value>() {
                let (key, value) = pair.map_err(|error| error.to_string())?;
                let lua::Value::String(key) = key else {
                    return Err("UI document object keys must be strings".to_owned());
                };
                values.insert(key.to_string_lossy(), lua_value_to_json(value, depth + 1)?);
            }
            Ok(serde_json::Value::Object(values))
        }
        value => Err(format!(
            "UI documents cannot contain {} values",
            value.type_name()
        )),
    }
}

fn json_to_lua(lua: &lua::Lua, value: &serde_json::Value, depth: usize) -> lua::Result<lua::Value> {
    if depth > 32 {
        return Err(lua::Error::runtime(
            "network message nesting exceeds 32 levels",
        ));
    }
    match value {
        serde_json::Value::Null => Ok(lua::Value::Nil),
        serde_json::Value::Bool(value) => Ok(lua::Value::Boolean(*value)),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(lua::Value::Integer)
            .or_else(|| value.as_f64().map(lua::Value::Number))
            .ok_or_else(|| lua::Error::runtime("network message number is not finite")),
        serde_json::Value::String(value) => {
            #[cfg(not(target_arch = "wasm32"))]
            let value = lua.create_string(value)?;
            #[cfg(target_arch = "wasm32")]
            let value = lua.create_string(value);
            Ok(lua::Value::String(value))
        }
        serde_json::Value::Array(values) => {
            let table = create_table(lua)?;
            for (index, value) in values.iter().enumerate() {
                table.set(index + 1, json_to_lua(lua, value, depth + 1)?)?;
            }
            Ok(lua::Value::Table(table))
        }
        serde_json::Value::Object(values) => {
            let table = create_table(lua)?;
            for (key, value) in values {
                table.set(key.as_str(), json_to_lua(lua, value, depth + 1)?)?;
            }
            Ok(lua::Value::Table(table))
        }
    }
}

fn create_table(lua: &lua::Lua) -> lua::Result<lua::Table> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        lua.create_table()
    }

    #[cfg(target_arch = "wasm32")]
    {
        Ok(lua.create_table())
    }
}

#[cfg(test)]
mod tests {
    use super::{GameScript, InteractionScriptState, InteractionZoneState};
    use crate::ui::{UiInsets, UiPointerPhase, UiRuntime, UiViewport};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn load(source: &str) -> (GameScript, Rc<RefCell<UiRuntime>>) {
        let ui = Rc::new(RefCell::new(UiRuntime::default()));
        let script = GameScript::load(source, Rc::clone(&ui)).expect("script should load");
        (script, ui)
    }

    #[test]
    fn configured_external_game_script_compiles() {
        let Ok(path) = std::env::var("CUBACADABRA_TEST_GAME_SCRIPT") else {
            return;
        };
        let source = std::fs::read_to_string(&path).expect("configured game script should exist");
        load(&source);
    }

    #[test]
    fn loads_luau_lifecycle_callbacks() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.lobby:set_status("ready")
                end
                function game.on_tick(_api, _delta)
                end
                return game
            "#,
        );

        assert_eq!(script.state().borrow().lobby_status, "ready");
        script.tick(1.0 / 60.0).expect("tick should run");
    }

    #[test]
    fn luau_receives_obby_player_events() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_player_event(api, event)
                    api.lobby:set_status(event.kind .. ":" .. event.deaths)
                end
                return game
            "#,
        );

        script
            .player_event(&crate::types::PlayerEvent::Death {
                cause: "fall".to_owned(),
                checkpoint: "tower".to_owned(),
                deaths: 3,
            })
            .expect("player event callback should run");
        assert_eq!(script.state().borrow().lobby_status, "death:3");
    }

    #[test]
    fn luau_can_disable_the_lobby() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.lobby:set_enabled(false)
                end
                return game
            "#,
        );

        assert_eq!(script.lobby_enabled_override(), Some(false));
    }

    #[test]
    fn luau_can_read_generic_interactions_and_receive_events() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_interaction(api, event)
                    api.lobby:set_status(event.id .. ":" .. event.phase)
                end
                function game.on_tick(api)
                    local state = api.interactions:get_state()
                    if state.zones.button.inside then
                        api.lobby:set_status("inside:" .. state.zones.button.players)
                    end
                end
                return game
            "#,
        );

        script.set_interaction_state(InteractionScriptState {
            zones: vec![InteractionZoneState {
                id: "button".to_owned(),
                kind: "zone".to_owned(),
                label: "BUTTON".to_owned(),
                inside: true,
                nearby: true,
                players: 2,
            }],
            event_id: 1,
        });
        script
            .interaction(&crate::types::InteractionEvent {
                id: "button".to_owned(),
                phase: "enter".to_owned(),
                players: 2,
            })
            .expect("interaction callback should run");
        assert_eq!(script.state().borrow().lobby_status, "button:enter");
        script.tick(0.0).expect("tick should run");
        assert_eq!(script.state().borrow().lobby_status, "inside:2");
    }

    #[test]
    fn luau_can_publish_and_receive_opaque_network_messages() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.network:set_state("progress", { learned = 2 })
                end
                function game.on_network_message(api, message)
                    if message.type == "game_state" and message.channel == "progress" then
                        api.lobby:set_status("learned:" .. message.payload.learned)
                    end
                end
                return game
            "#,
        );

        script
            .take_network_message()
            .expect("on_start should emit a retained state message");
        assert!(script.enqueue_network_message(
            r#"{"type":"game_state","channel":"progress","payload":{"learned":3}}"#,
        ));
        script.tick(0.0).expect("network callback should run");
        assert_eq!(script.state().borrow().lobby_status, "learned:3");
    }

    #[test]
    fn luau_can_compare_and_set_authoritative_state() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.network:compare_set_state("round", 7, {
                        round = 3,
                        checkpoints = { gate_a = true },
                    })
                end
                return game
            "#,
        );

        let message = script
            .take_network_message()
            .expect("compare_set_state should emit a network command");
        let value: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(value["channel"], "round");
        assert_eq!(value["expectedSequence"], 7);
        assert_eq!(value["payload"]["round"], 3);
        assert_eq!(value["payload"]["checkpoints"]["gate_a"], true);
    }

    #[test]
    fn luau_can_control_manifest_defined_effects() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.effects:set_state("gate-a", "open")
                    api.effects:play("finish-flash", { position = { 1, 2, 3 } })
                end
                return game
            "#,
        );

        assert_eq!(
            script.take_effect_commands(),
            vec![
                crate::effects::EffectCommand::SetState {
                    target: "gate-a".to_owned(),
                    state: "open".to_owned(),
                },
                crate::effects::EffectCommand::Play {
                    template: "finish-flash".to_owned(),
                    position: [1.0, 2.0, 3.0],
                },
            ]
        );
    }

    #[test]
    fn script_queues_are_bounded() {
        let (script, _) = load("return {}");
        let message = format!(r#"{{"payload":"{}"}}"#, "x".repeat(4096));
        let mut accepted = 0;
        while script.enqueue_network_message(&message) {
            accepted += 1;
        }
        assert!(accepted > 0);
        assert!(accepted < 64);
    }

    #[test]
    fn luau_can_emit_bounded_game_audio_commands() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.audio:play("success-chime", { volume = 0.75 })
                end
                return game
            "#,
        );

        let message = script
            .take_audio_message()
            .expect("on_start should emit an audio command");
        let value: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(value["type"], "play");
        assert_eq!(value["id"], "success-chime");
        assert_eq!(value["volume"], 0.75);
    }

    #[test]
    fn audio_overflow_keeps_the_newest_commands_without_stopping_the_game() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    for index = 1, 80 do
                        api.audio:play("sound-" .. index)
                    end
                    api.lobby:set_status("still running")
                end
                return game
            "#,
        );

        assert_eq!(script.state().borrow().lobby_status, "still running");
        let mut messages = Vec::new();
        while let Some(message) = script.take_audio_message() {
            messages.push(serde_json::from_str::<serde_json::Value>(&message).unwrap());
        }
        assert_eq!(messages.len(), 64);
        assert_eq!(messages.first().unwrap()["id"], "sound-17");
        assert_eq!(messages.last().unwrap()["id"], "sound-80");
    }

    #[test]
    fn launch_callback_receives_pad_and_players() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_launch(api, launch)
                    api.session:start(launch.pad_id .. ":" .. #launch.player_ids, {})
                end
                return game
            "#,
        );

        script.launch("pad-2", &[4, 9]).expect("launch should run");
        assert_eq!(
            script.state().borrow().session_name.as_deref(),
            Some("pad-2:2")
        );
    }

    #[test]
    fn luau_can_declare_ui_and_receive_actions() {
        let (script, ui) = load(
            r##"
                local game = {}
                function game.on_start(api)
                    api.ui:set_document([[{
                        "nodes":[{
                            "id":"settings",
                            "kind":"button",
                            "text":"SETTINGS",
                            "action":"open-settings",
                            "layout":{"width":120,"height":48}
                        }]
                    }]])
                end
                function game.on_ui_event(api, event)
                    if event.action == "open-settings" then
                        api.lobby:set_status("settings-open")
                    end
                end
                return game
            "##,
        );
        ui.borrow_mut().set_viewport(UiViewport {
            width: 390.0,
            height: 844.0,
            scale: 1.0,
            safe_area: UiInsets::default(),
        });
        assert!(ui.borrow_mut().pointer(1, UiPointerPhase::Down, 10.0, 10.0));
        assert!(ui.borrow_mut().pointer(1, UiPointerPhase::Up, 10.0, 10.0));
        script.tick(0.0).unwrap();
        assert_eq!(script.state().borrow().lobby_status, "settings-open");
    }

    #[test]
    fn luau_can_declare_ui_with_tables() {
        let (_, ui) = load(
            r##"
                local game = {}
                function game.on_start(api)
                    api.ui:set_document({
                        nodes = {
                            {
                                id = "dock",
                                kind = "panel",
                                layout = {
                                    anchor = "bottom",
                                    width = "90%",
                                    maxWidth = 520,
                                    height = 60,
                                },
                                children = {
                                    { id = "place", kind = "button", text = "PLACE" },
                                },
                            },
                        },
                    })
                end
                return game
            "##,
        );
        ui.borrow_mut().set_viewport(UiViewport {
            width: 390.0,
            height: 844.0,
            scale: 1.0,
            safe_area: UiInsets::default(),
        });
        let frame = ui.borrow_mut().frame().clone();
        assert_eq!(frame.nodes[0].id, "dock");
        assert_eq!(frame.nodes[1].id, "place");
        assert_eq!(frame.nodes[0].rect.width, 351.0);
    }
}
