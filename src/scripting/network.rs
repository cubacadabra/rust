use std::cell::RefCell;
use std::rc::Rc;

use super::{ScriptState, create_table, lua, lua_value_to_json, queue_has_capacity};

pub(super) fn install(
    lua: &lua::Lua,
    api: &lua::Table,
    state: Rc<RefCell<ScriptState>>,
) -> lua::Result<()> {
    let network = create_table(lua)?;
    for (method, retained) in [("publish", false), ("set_state", true)] {
        let network_state = Rc::clone(&state);
        network.set(
            method,
            lua.create_function(
                move |_, (_network, channel, payload): (lua::Table, String, lua::Value)| {
                    enqueue(&network_state, channel, payload, retained, None)
                },
            )?,
        )?;
    }

    let compare_state = state;
    network.set(
        "compare_set_state",
        lua.create_function(
            move |_, (_network, channel, expected, payload): (
                lua::Table,
                String,
                u32,
                lua::Value,
            )| {
                enqueue(&compare_state, channel, payload, true, Some(expected))
            },
        )?,
    )?;
    api.set("network", network)
}

fn enqueue(
    state: &Rc<RefCell<ScriptState>>,
    channel: String,
    payload: lua::Value,
    retained: bool,
    expected_sequence: Option<u32>,
) -> lua::Result<()> {
    if channel.is_empty() || channel.len() > crate::engine::identity::MAX_NETWORK_CHANNEL_BYTES {
        return Err(lua::Error::runtime("network channel must be 1–64 bytes"));
    }
    let payload = lua_value_to_json(payload, 0).map_err(lua::Error::runtime)?;
    let mut message = serde_json::json!({
        "channel": channel,
        "payload": payload,
        "retained": retained,
    });
    if let Some(expected_sequence) = expected_sequence {
        message["expectedSequence"] = expected_sequence.into();
    }
    let source =
        serde_json::to_string(&message).map_err(|error| lua::Error::runtime(error.to_string()))?;
    if source.len() > crate::engine::identity::MAX_NETWORK_MESSAGE_BYTES {
        return Err(lua::Error::runtime("network message exceeds 64 KiB"));
    }
    let mut state = state.borrow_mut();
    if !queue_has_capacity(&state.network_outbox, source.len()) {
        return Err(lua::Error::runtime("network command queue is full"));
    }
    state.network_outbox.push_back(source);
    Ok(())
}
