use std::cell::RefCell;
use std::rc::Rc;

use crate::effects::{EffectCommand, MAX_EFFECT_COMMANDS, valid_effect_id};

use super::{ScriptState, create_table, lua};

pub(super) fn install(
    lua: &lua::Lua,
    api: &lua::Table,
    state: Rc<RefCell<ScriptState>>,
) -> lua::Result<()> {
    let effects = create_table(lua)?;

    let state_commands = Rc::clone(&state);
    effects.set(
        "set_state",
        lua.create_function(
            move |_, (_effects, target, value): (lua::Table, String, String)| {
                validate_name("effect target", &target)?;
                validate_name("effect state", &value)?;
                enqueue(
                    &state_commands,
                    EffectCommand::SetState {
                        target,
                        state: value,
                    },
                )
            },
        )?,
    )?;

    effects.set(
        "play",
        lua.create_function(
            move |_, (_effects, template, options): (lua::Table, String, Option<lua::Table>)| {
                validate_name("effect template", &template)?;
                let position = options
                    .as_ref()
                    .map(|options| options.get::<Option<lua::Table>>("position"))
                    .transpose()?
                    .flatten()
                    .map(|value| {
                        Ok::<_, lua::Error>([
                            value.get::<Option<f32>>(1)?.unwrap_or(0.0),
                            value.get::<Option<f32>>(2)?.unwrap_or(0.0),
                            value.get::<Option<f32>>(3)?.unwrap_or(0.0),
                        ])
                    })
                    .transpose()?
                    .unwrap_or([0.0; 3]);
                let travel_to = options
                    .as_ref()
                    .map(|options| options.get::<Option<lua::Table>>("travelTo"))
                    .transpose()?
                    .flatten()
                    .map(|value| {
                        Ok::<_, lua::Error>([
                            value.get::<Option<f32>>(1)?.unwrap_or(0.0),
                            value.get::<Option<f32>>(2)?.unwrap_or(0.0),
                            value.get::<Option<f32>>(3)?.unwrap_or(0.0),
                        ])
                    })
                    .transpose()?;
                if !position
                    .iter()
                    .all(|value| value.is_finite() && value.abs() <= 10_000.0)
                {
                    return Err(lua::Error::runtime(
                        "effect position must contain finite world coordinates",
                    ));
                }
                if travel_to.is_some_and(|destination| {
                    !destination
                        .iter()
                        .all(|value| value.is_finite() && value.abs() <= 10_000.0)
                }) {
                    return Err(lua::Error::runtime(
                        "effect travelTo must contain finite relative coordinates",
                    ));
                }
                enqueue(
                    &state,
                    EffectCommand::Play {
                        template,
                        position,
                        travel_to,
                    },
                )
            },
        )?,
    )?;

    api.set("effects", effects)
}

fn validate_name(label: &str, value: &str) -> lua::Result<()> {
    if valid_effect_id(value) {
        Ok(())
    } else {
        Err(lua::Error::runtime(format!(
            "{label} must be 1–64 ASCII letters, numbers, dots, dashes, or underscores"
        )))
    }
}

fn enqueue(state: &Rc<RefCell<ScriptState>>, command: EffectCommand) -> lua::Result<()> {
    let mut state = state.borrow_mut();
    if state.effect_outbox.len() >= MAX_EFFECT_COMMANDS {
        return Err(lua::Error::runtime("effect command queue is full"));
    }
    state.effect_outbox.push_back(command);
    Ok(())
}
