use std::{collections::VecDeque, ptr, slice};

use crate::{AppAction, AppEffect, AppModel, EffectId, UsernameSaveError};

pub struct CubacadabraApp {
    model: AppModel,
    pending_effects: VecDeque<AppEffect>,
    output_buffer: Vec<u8>,
    effect_id: EffectId,
}

unsafe fn source<'a>(pointer: *const u8, length: usize) -> Option<&'a str> {
    if pointer.is_null() && length != 0 {
        return None;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(pointer, length) }
    };
    std::str::from_utf8(bytes).ok()
}

fn replace_output(app: &mut CubacadabraApp, value: &impl serde::Serialize) -> bool {
    match serde_json::to_vec(value) {
        Ok(output) => {
            app.output_buffer = output;
            true
        }
        Err(_) => {
            app.output_buffer.clear();
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_create(
    username: *const u8,
    username_length: usize,
) -> *mut CubacadabraApp {
    let Some(username) = (unsafe { source(username, username_length) }) else {
        return ptr::null_mut();
    };
    let username = (!username.is_empty()).then(|| username.to_owned());
    Box::into_raw(Box::new(CubacadabraApp {
        model: AppModel::new(username),
        pending_effects: VecDeque::new(),
        output_buffer: Vec::new(),
        effect_id: EffectId(0),
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_destroy(app: *mut CubacadabraApp) {
    if !app.is_null() {
        drop(unsafe { Box::from_raw(app) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_replace_profile(
    app: *mut CubacadabraApp,
    username: *const u8,
    username_length: usize,
) -> u8 {
    let Some(username) = (unsafe { source(username, username_length) }) else {
        return 0;
    };
    let username = (!username.is_empty()).then(|| username.to_owned());
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    app.model.dispatch(AppAction::ReplaceProfile { username });
    app.pending_effects.clear();
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_username_changed(
    app: *mut CubacadabraApp,
    value: *const u8,
    value_length: usize,
) -> u8 {
    let Some(value) = (unsafe { source(value, value_length) }) else {
        return 0;
    };
    let value = value.to_owned();
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    app.model.dispatch(AppAction::UsernameChanged { value });
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_save_username(app: *mut CubacadabraApp) {
    if let Some(app) = unsafe { app.as_mut() } {
        app.model.dispatch(AppAction::SaveUsername);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_username_saved(
    app: *mut CubacadabraApp,
    effect_id: u32,
    username: *const u8,
    username_length: usize,
) -> u8 {
    let Some(username) = (unsafe { source(username, username_length) }) else {
        return 0;
    };
    let username = username.to_owned();
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    app.model.dispatch(AppAction::UsernameSaved {
        effect_id: EffectId(effect_id),
        username,
    });
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_username_save_failed(
    app: *mut CubacadabraApp,
    effect_id: u32,
    server_code: *const u8,
    server_code_length: usize,
) -> u8 {
    let Some(server_code) = (unsafe { source(server_code, server_code_length) }) else {
        return 0;
    };
    let error = UsernameSaveError::from_server_code(server_code);
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    app.model.dispatch(AppAction::UsernameSaveFailed {
        effect_id: EffectId(effect_id),
        error,
    });
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_clear_username_feedback(app: *mut CubacadabraApp) {
    if let Some(app) = unsafe { app.as_mut() } {
        app.model.dispatch(AppAction::ClearUsernameFeedback);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_snapshot_json(app: *mut CubacadabraApp) -> u8 {
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    replace_output(app, &app.model.snapshot()).into()
}

/// Returns 0 for no work and 1 for a username-save effect.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_poll_effect(app: *mut CubacadabraApp) -> u8 {
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    if app.pending_effects.is_empty() {
        app.pending_effects.extend(app.model.take_effects());
    }
    let Some(effect) = app.pending_effects.pop_front() else {
        app.output_buffer.clear();
        app.effect_id = EffectId(0);
        return 0;
    };
    match effect {
        AppEffect::SaveUsername {
            effect_id,
            username,
        } => {
            app.effect_id = effect_id;
            app.output_buffer = username.into_bytes();
            1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_effect_id(app: *const CubacadabraApp) -> u32 {
    unsafe { app.as_ref() }
        .map(|app| app.effect_id.0)
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_output_ptr(app: *const CubacadabraApp) -> *const u8 {
    unsafe { app.as_ref() }
        .map(|app| app.output_buffer.as_ptr())
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_output_len(app: *const CubacadabraApp) -> usize {
    unsafe { app.as_ref() }
        .map(|app| app.output_buffer.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_adapter_runs_the_username_effect_cycle() {
        let initial = b"Ada";
        let app = unsafe { cubacadabra_app_create(initial.as_ptr(), initial.len()) };
        assert!(!app.is_null());

        let draft = b"Grace_7";
        assert_eq!(
            unsafe { cubacadabra_app_username_changed(app, draft.as_ptr(), draft.len()) },
            1
        );
        unsafe { cubacadabra_app_save_username(app) };
        assert_eq!(unsafe { cubacadabra_app_poll_effect(app) }, 1);
        let effect_id = unsafe { cubacadabra_app_effect_id(app) };
        let output = unsafe {
            slice::from_raw_parts(
                cubacadabra_app_output_ptr(app),
                cubacadabra_app_output_len(app),
            )
        };
        assert_eq!(output, draft);

        assert_eq!(
            unsafe {
                cubacadabra_app_username_saved(app, effect_id, output.as_ptr(), output.len())
            },
            1
        );
        assert_eq!(unsafe { cubacadabra_app_snapshot_json(app) }, 1);
        let snapshot = unsafe {
            slice::from_raw_parts(
                cubacadabra_app_output_ptr(app),
                cubacadabra_app_output_len(app),
            )
        };
        let snapshot: serde_json::Value = serde_json::from_slice(snapshot).expect("snapshot JSON");
        assert_eq!(snapshot["profile"]["username"], "Grace_7");

        unsafe { cubacadabra_app_destroy(app) };
    }
}
