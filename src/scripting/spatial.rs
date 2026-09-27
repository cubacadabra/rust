//! Opt-in observations and game-authored kinematic displacement of pushable bodies.
use super::{ScriptState, create_table, lua};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Clone, Debug)]
pub(crate) struct BodyObservation {
    pub position: [f32; 3],
    pub offset: [f32; 2],
    pub shift: [f32; 3],
}

#[derive(Default, Debug)]
pub(crate) struct SpatialState {
    pub watched: BTreeMap<String, Option<BodyObservation>>,
    pub player: Option<[f32; 3]>,
    pub shifts: BTreeMap<String, [f32; 3]>,
    pub impulse: [f32; 3],
}

fn vector(value: lua::Table, limit: f32) -> lua::Result<[f32; 3]> {
    let value = [
        value.get::<f32>(1)?,
        value.get::<f32>(2)?,
        value.get::<f32>(3)?,
    ];
    if value.iter().any(|v| !v.is_finite() || v.abs() > limit) {
        return Err(lua::Error::runtime(
            "spatial vector is non-finite or out of bounds",
        ));
    }
    Ok(value)
}

pub(super) fn install(
    lua: &lua::Lua,
    api: &lua::Table,
    state: Rc<RefCell<ScriptState>>,
) -> lua::Result<()> {
    let world: lua::Table = api.get("world")?;
    let watch = Rc::clone(&state);
    world.set(
        "watch_blocks",
        lua.create_function(move |_, (_world, ids): (lua::Table, lua::Table)| {
            let mut watched = BTreeMap::new();
            for (index, id) in ids.sequence_values::<String>().enumerate() {
                let id = id?;
                if index >= 64 || id.trim().is_empty() || id.len() > 128 {
                    return Err(lua::Error::runtime(
                        "watch_blocks accepts up to 64 IDs of 1-128 bytes",
                    ));
                }
                watched.insert(id, None);
            }
            let mut state = watch.borrow_mut();
            state.spatial.watched = watched;
            state.spatial.player = None;
            Ok(())
        })?,
    )?;
    let read = Rc::clone(&state);
    world.set(
        "get_positions",
        lua.create_function(move |lua, _world: lua::Table| {
            let state = read.borrow();
            let Some(player) = state.spatial.player else {
                return Ok(lua::Value::Nil);
            };
            let result = create_table(lua)?;
            result.set("worldId", state.world_id.as_str())?;
            result.set("player", lua.create_sequence_from(player)?)?;
            let blocks = create_table(lua)?;
            for (id, body) in &state.spatial.watched {
                if let Some(body) = body {
                    let entry = create_table(lua)?;
                    entry.set("position", lua.create_sequence_from(body.position)?)?;
                    entry.set("offset", lua.create_sequence_from(body.offset)?)?;
                    entry.set("shift", lua.create_sequence_from(body.shift)?)?;
                    blocks.set(id.as_str(), entry)?;
                }
            }
            result.set("blocks", blocks)?;
            Ok(lua::Value::Table(result))
        })?,
    )?;
    let shift = Rc::clone(&state);
    world.set(
        "set_block_shift",
        lua.create_function(
            move |_, (_world, id, value): (lua::Table, String, lua::Table)| {
                let value = vector(value, 512.0)?;
                let mut state = shift.borrow_mut();
                if !state.spatial.watched.get(&id).is_some_and(Option::is_some) {
                    return Ok(false);
                }
                state.spatial.shifts.insert(id, value);
                Ok(true)
            },
        )?,
    )?;
    let player = create_table(lua)?;
    player.set(
        "impulse",
        lua.create_function(move |_, (_player, value): (lua::Table, lua::Table)| {
            let value = vector(value, 30.0)?;
            let mut state = state.borrow_mut();
            for (pending, value) in state.spatial.impulse.iter_mut().zip(value) {
                *pending = (*pending + value).clamp(-30.0, 30.0);
            }
            Ok(())
        })?,
    )?;
    api.set("player", player)
}
