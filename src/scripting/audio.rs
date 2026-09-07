use std::cell::RefCell;
use std::rc::Rc;

use super::{ScriptState, create_table, lua, queue_has_capacity};

const MAX_AUDIO_ID_BYTES: usize = 64;

pub(super) fn install(
    lua: &lua::Lua,
    api: &lua::Table,
    state: Rc<RefCell<ScriptState>>,
) -> lua::Result<()> {
    let audio = create_table(lua)?;
    audio.set(
        "play",
        lua.create_function(
            move |_, (_audio, id, options): (lua::Table, String, Option<lua::Table>)| {
                if !valid_audio_id(&id) {
                    return Err(lua::Error::runtime(
                        "audio id must be 1–64 ASCII letters, numbers, dots, dashes, or underscores",
                    ));
                }
                let volume = options
                    .as_ref()
                    .and_then(|value| value.get::<Option<f32>>("volume").ok().flatten())
                    .unwrap_or(1.0);
                if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
                    return Err(lua::Error::runtime("audio volume must be between 0 and 1"));
                }
                let message = serde_json::to_string(&serde_json::json!({
                    "type": "play",
                    "id": id,
                    "volume": volume,
                }))
                .map_err(|error| lua::Error::runtime(error.to_string()))?;
                let mut state = state.borrow_mut();
                while !queue_has_capacity(&state.audio_outbox, message.len()) {
                    if state.audio_outbox.pop_front().is_none() {
                        return Ok(());
                    }
                }
                state.audio_outbox.push_back(message);
                Ok(())
            },
        )?,
    )?;
    api.set("audio", audio)
}

fn valid_audio_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_AUDIO_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}
