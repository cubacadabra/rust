use std::cell::RefCell;
use std::rc::Rc;

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
    state: Rc<RefCell<ScriptState>>,
}

impl GameScript {
    pub(crate) fn load(source: &str) -> Result<Self, String> {
        Self::load_inner(source).map_err(|error| error.to_string())
    }

    fn load_inner(source: &str) -> lua::Result<Self> {
        let lua = lua::Lua::new();
        lua.sandbox(true)?;
        let state = Rc::new(RefCell::new(ScriptState::default()));
        let api = create_api(&lua, Rc::clone(&state))?;
        let module: lua::Table = lua.load(source).set_name("game.luau").eval()?;
        let on_start: Option<lua::Function> = module.get("on_start")?;
        let on_tick: Option<lua::Function> = module.get("on_tick")?;
        let on_launch: Option<lua::Function> = module.get("on_launch")?;

        if let Some(on_start) = on_start {
            on_start.call::<()>((api.clone(),))?;
        }

        Ok(Self {
            lua,
            api,
            on_tick,
            on_launch,
            state,
        })
    }

    pub(crate) fn tick(&self, delta: f32) -> Result<(), String> {
        if let Some(on_tick) = &self.on_tick {
            on_tick
                .call::<()>((self.api.clone(), delta))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
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

fn create_api(lua: &lua::Lua, state: Rc<RefCell<ScriptState>>) -> lua::Result<lua::Table> {
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

    Ok(api)
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

    #[test]
    fn loads_luau_lifecycle_callbacks() {
        let script = GameScript::load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.lobby:set_status("ready")
                end
                function game.on_tick(_api, _delta)
                end
                return game
            "#,
        )
        .expect("script should load");

        assert_eq!(script.state().borrow().lobby_status, "ready");
        script.tick(1.0 / 60.0).expect("tick should run");
    }

    #[test]
    fn launch_callback_receives_pad_and_players() {
        let script = GameScript::load(
            r#"
                local game = {}
                function game.on_launch(api, launch)
                    api.session:start(launch.pad_id .. ":" .. #launch.player_ids, {})
                end
                return game
            "#,
        )
        .expect("script should load");

        script.launch("pad-2", &[4, 9]).expect("launch should run");
        assert_eq!(
            script.state().borrow().session_name.as_deref(),
            Some("pad-2:2")
        );
    }
}
