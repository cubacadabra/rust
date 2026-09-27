use std::cell::RefCell;
use std::rc::Rc;

use crate::schema::{ClassSchema, PropertyValue, interaction_zone_registry};
use crate::ui::UiRuntime;

use super::{
    ScriptState, TaskScheduler, audio, create_table, effects, lua, network, scheduler,
    ui_document_source,
};
#[cfg(debug_assertions)]
use super::DebugTeleportRequest;

pub(super) fn create_api(
    lua: &lua::Lua,
    state: Rc<RefCell<ScriptState>>,
    ui_runtime: Rc<RefCell<UiRuntime>>,
    scheduler: Rc<RefCell<TaskScheduler>>,
) -> lua::Result<lua::Table> {
    let api = create_table(lua)?;
    api.set(
        "build_mode",
        if cfg!(debug_assertions) {
            "DEBUG"
        } else {
            "RELEASE"
        },
    )?;

    #[cfg(debug_assertions)]
    {
        let debug = create_table(lua)?;
        let debug_state = Rc::clone(&state);
        debug.set(
            "teleport_to",
            lua.create_function(
                move |_, (_debug, world_id, position, yaw): (lua::Table, String, lua::Table, f32)| {
                    let position = [
                        position.get::<f32>(1)?,
                        position.get::<f32>(2)?,
                        position.get::<f32>(3)?,
                    ];
                    if !position.iter().all(|value| value.is_finite()) || !yaw.is_finite() {
                        return Err(lua::Error::runtime(
                            "debug:teleport_to expects finite coordinates and yaw",
                        ));
                    }
                    debug_state.borrow_mut().debug_teleport = Some(DebugTeleportRequest {
                        world_id,
                        position,
                        yaw,
                    });
                    Ok(())
                },
            )?,
        )?;
        api.set("debug", debug)?;
    }

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

    let world = create_table(lua)?;
    let world_state = Rc::clone(&state);
    world.set(
        "get_id",
        lua.create_function(move |_, _world: lua::Table| {
            Ok(world_state.borrow().world_id.clone())
        })?,
    )?;
    let world_state = Rc::clone(&state);
    world.set(
        "enter",
        lua.create_function(move |_, (_world, world_id): (lua::Table, String)| {
            if world_id.trim().is_empty() {
                return Err(lua::Error::runtime(
                    "world:enter requires a non-empty package world id",
                ));
            }
            let mut state = world_state.borrow_mut();
            if !state
                .world_ids
                .iter()
                .any(|candidate| candidate == &world_id)
            {
                return Err(lua::Error::runtime(format!(
                    "world:enter cannot find package world `{world_id}`",
                )));
            }
            if state.world_transition.is_some() {
                return Err(lua::Error::runtime(
                    "world:enter already has a transition queued",
                ));
            }
            state.world_transition = Some(world_id);
            Ok(())
        })?,
    )?;
    api.set("world", world)?;
    install_spatial_api(lua, &api, Rc::clone(&state))?;

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
    interactions.set(
        "get_schema",
        lua.create_function(
            move |lua, (_interactions, class_id): (lua::Table, String)| {
                let registry = interaction_zone_registry();
                let Some(class) = registry.class(&class_id) else {
                    return Ok(lua::Value::Nil);
                };
                interaction_class_to_lua(lua, class).map(lua::Value::Table)
            },
        )?,
    )?;
    api.set("interactions", interactions)?;

    scheduler::install(lua, &api, scheduler)?;
    network::install(lua, &api, Rc::clone(&state))?;
    audio::install(lua, &api, Rc::clone(&state))?;
    effects::install(lua, &api, Rc::clone(&state))?;
    Ok(api)
}

fn install_spatial_api(
    lua: &lua::Lua,
    api: &lua::Table,
    state: Rc<RefCell<ScriptState>>,
) -> lua::Result<()> {
    let world: lua::Table = api.get("world")?;
    let watch_state = Rc::clone(&state);
    world.set("watch_positions", lua.create_function(
        move |_, (_world, ids): (lua::Table, lua::Table)| {
            let mut watched = std::collections::BTreeMap::new();
            for (index, id) in ids.sequence_values::<String>().enumerate() {
                let id = id?;
                if id.trim().is_empty() || id.len() > 128 || index >= 64 {
                    return Err(lua::Error::runtime("watch_positions requires at most 64 non-empty block IDs (up to 128 bytes each)"));
                }
                watched.insert(id, None);
            }
            let mut state = watch_state.borrow_mut();
            state.watched_blocks = watched;
            state.watched_player = None;
            Ok(())
        }
    )?)?;
    world.set("get_positions", lua.create_function(move |lua, _world: lua::Table| {
        let state = state.borrow();
        let Some(player) = state.watched_player else { return Ok(lua::Value::Nil); };
        let result = create_table(lua)?;
        result.set("worldId", state.world_id.as_str())?;
        result.set("player", lua.create_sequence_from(player)?)?;
        let blocks = create_table(lua)?;
        for (id, position) in &state.watched_blocks {
            if let Some(position) = position {
                blocks.set(id.as_str(), lua.create_sequence_from(*position)?)?;
            }
        }
        result.set("blocks", blocks)?;
        Ok(lua::Value::Table(result))
    })?)?;
    Ok(())
}

fn interaction_class_to_lua(lua: &lua::Lua, class: &ClassSchema) -> lua::Result<lua::Table> {
    let result = create_table(lua)?;
    result.set("id", class.id.as_str())?;
    result.set("name", class.name.as_str())?;
    let properties = create_table(lua)?;
    for (index, property) in class.properties.iter().enumerate() {
        let value = create_table(lua)?;
        value.set("id", property.id.as_str())?;
        value.set("name", property.name.as_str())?;
        value.set("type", property.value_type.as_str())?;
        value.set("default", property_value_to_lua(lua, &property.default)?)?;
        if let Some(min) = property.range.min {
            value.set("min", min)?;
        }
        if let Some(max) = property.range.max {
            value.set("max", max)?;
        }
        let flags = create_table(lua)?;
        flags.set("scriptVisible", property.flags.script_visible)?;
        flags.set("serializable", property.flags.serializable)?;
        flags.set("editorVisible", property.flags.editor_visible)?;
        flags.set("replicable", property.flags.replicable)?;
        value.set("flags", flags)?;
        properties.set(index + 1, value)?;
    }
    result.set("properties", properties)?;
    Ok(result)
}

fn property_value_to_lua(lua: &lua::Lua, value: &PropertyValue) -> lua::Result<lua::Value> {
    match value {
        PropertyValue::String(value) => {
            #[cfg(not(target_arch = "wasm32"))]
            let value = lua.create_string(value)?;
            #[cfg(target_arch = "wasm32")]
            let value = lua.create_string(value);
            Ok(lua::Value::String(value))
        }
        PropertyValue::Number(value) => Ok(lua::Value::Number(f64::from(*value))),
        PropertyValue::Vector3(values) => Ok(lua::Value::Table(
            lua.create_sequence_from(values.iter().copied())?,
        )),
    }
}
