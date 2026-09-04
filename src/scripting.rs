use std::cell::RefCell;
use std::rc::Rc;

use crate::ui::{UiEvent, UiRuntime};

#[cfg(target_arch = "wasm32")]
use luaur_rt as lua;
#[cfg(not(target_arch = "wasm32"))]
use mlua as lua;

#[allow(dead_code)]
#[derive(Default, Debug)]
pub(crate) struct ScriptState {
    pub(crate) lobby_status: String,
    pub(crate) session_name: Option<String>,
    pub(crate) last_error: Option<String>,
}

pub(crate) struct GameScript {
    lua: lua::Lua,
    api: lua::Table,
    on_tick: Option<lua::Function>,
    on_launch: Option<lua::Function>,
    on_ui_event: Option<lua::Function>,
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

        if let Some(on_start) = on_start {
            on_start.call::<()>((api.clone(),))?;
        }

        Ok(Self {
            lua,
            api,
            on_tick,
            on_launch,
            on_ui_event,
            state,
            ui,
        })
    }

    pub(crate) fn tick(&self, delta: f32) -> Result<(), String> {
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
    lobby.set(
        "set_status",
        lua.create_function(move |_, (_lobby, status): (lua::Table, String)| {
            lobby_state.borrow_mut().lobby_status = status;
            Ok(())
        })?,
    )?;
    api.set("lobby", lobby)?;

    let session = create_table(lua)?;
    let session_state = state;
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

    Ok(api)
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
    use super::GameScript;
    use crate::ui::{UiInsets, UiPointerPhase, UiRuntime, UiViewport};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn load(source: &str) -> (GameScript, Rc<RefCell<UiRuntime>>) {
        let ui = Rc::new(RefCell::new(UiRuntime::default()));
        let script = GameScript::load(source, Rc::clone(&ui)).expect("script should load");
        (script, ui)
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
