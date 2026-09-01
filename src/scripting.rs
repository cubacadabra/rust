use std::cell::RefCell;
use std::rc::Rc;

#[allow(dead_code)]
#[derive(Default, Debug)]
pub(crate) struct ScriptState {
    pub(crate) lobby_status: String,
    pub(crate) session_name: Option<String>,
    pub(crate) last_error: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::{Rc, RefCell, ScriptState};
    use mlua::{Function, Lua, Result as LuaResult, Table};

    pub(crate) struct GameScript {
        lua: Lua,
        api: Table,
        on_tick: Option<Function>,
        on_launch: Option<Function>,
        state: Rc<RefCell<ScriptState>>,
    }

    impl GameScript {
        pub(crate) fn load(source: &str) -> Result<Self, String> {
            Self::load_inner(source).map_err(|error| error.to_string())
        }

        fn load_inner(source: &str) -> LuaResult<Self> {
            let lua = Lua::new();
            lua.sandbox(true)?;
            let state = Rc::new(RefCell::new(ScriptState::default()));
            let api = create_api(&lua, Rc::clone(&state))?;
            let module: Table = lua.load(source).set_name("game.luau").eval()?;
            let on_start: Option<Function> = module.get("on_start")?;
            let on_tick: Option<Function> = module.get("on_tick")?;
            let on_launch: Option<Function> = module.get("on_launch")?;

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
            let launch = self.lua.create_table().map_err(|error| error.to_string())?;
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

    fn create_api(lua: &Lua, state: Rc<RefCell<ScriptState>>) -> LuaResult<Table> {
        let api = lua.create_table()?;

        let lobby = lua.create_table()?;
        let lobby_state = Rc::clone(&state);
        lobby.set(
            "set_status",
            lua.create_function(move |_, (_lobby, status): (Table, String)| {
                lobby_state.borrow_mut().lobby_status = status;
                Ok(())
            })?,
        )?;
        api.set("lobby", lobby)?;

        let session = lua.create_table()?;
        let session_state = state;
        session.set(
            "start",
            lua.create_function(
                move |_, (_session, name, _options): (Table, String, Table)| {
                    session_state.borrow_mut().session_name = Some(name);
                    Ok(())
                },
            )?,
        )?;
        api.set("session", session)?;

        Ok(api)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::GameScript;

// The current browser engine is built for wasm32-unknown-unknown, which cannot
// link mlua's C++ Luau VM. Keep the ABI and engine lifecycle available while a
// dedicated Luau-WASM runtime is added; native clients already execute rules
// through the implementation above.
#[cfg(target_arch = "wasm32")]
pub(crate) struct GameScript {
    state: Rc<RefCell<ScriptState>>,
}

#[cfg(target_arch = "wasm32")]
impl GameScript {
    pub(crate) fn load(_source: &str) -> Result<Self, String> {
        Ok(Self {
            state: Rc::new(RefCell::new(ScriptState::default())),
        })
    }

    pub(crate) fn tick(&self, _delta: f32) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn launch(&self, _pad_id: &str, _player_ids: &[u32]) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn state(&self) -> Rc<RefCell<ScriptState>> {
        Rc::clone(&self.state)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::native::GameScript;

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
        .expect("test script should load");

        assert_eq!(script.state().borrow().lobby_status, "ready");
        script.tick(1.0 / 60.0).expect("tick should run");
    }
}
