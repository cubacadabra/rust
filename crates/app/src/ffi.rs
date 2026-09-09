use crate::{AppAction, AppModel};
use std::{ptr, slice};

#[derive(Default)]
pub struct CubacadabraApp {
    model: AppModel,
    output: Vec<u8>,
}

#[unsafe(no_mangle)]
pub extern "C" fn cubacadabra_app_create() -> *mut CubacadabraApp {
    Box::into_raw(Box::new(CubacadabraApp::default()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_destroy(app: *mut CubacadabraApp) {
    if !app.is_null() {
        drop(unsafe { Box::from_raw(app) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_dispatch_json(
    app: *mut CubacadabraApp,
    source: *const u8,
    length: usize,
) -> u8 {
    if source.is_null() && length != 0 {
        return 0;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(source, length) }
    };
    // Deserialize to owned values before borrowing app: input may alias output.
    let Ok(action) = serde_json::from_slice::<AppAction>(bytes) else {
        return 0;
    };
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    app.model.dispatch(action);
    app.output.clear();
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_snapshot_json(app: *mut CubacadabraApp) -> u8 {
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    app.output = serde_json::to_vec(&app.model.snapshot()).expect("app snapshots are serializable");
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_poll_effect_json(app: *mut CubacadabraApp) -> u8 {
    let Some(app) = (unsafe { app.as_mut() }) else {
        return 0;
    };
    let Some(effect) = app.model.poll_effect() else {
        app.output.clear();
        return 0;
    };
    app.output = serde_json::to_vec(&effect).expect("app effects are serializable");
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_output_ptr(app: *const CubacadabraApp) -> *const u8 {
    unsafe { app.as_ref() }
        .map(|app| app.output.as_ptr())
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cubacadabra_app_output_len(app: *const CubacadabraApp) -> usize {
    unsafe { app.as_ref() }
        .map(|app| app.output.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_input_without_mutating_and_copies_json_output() {
        unsafe {
            let app = cubacadabra_app_create();
            for invalid in [
                b"{".as_slice(),
                &[255],
                br#"{"type":"save_username","typo":true}"#,
            ] {
                assert_eq!(
                    cubacadabra_app_dispatch_json(app, invalid.as_ptr(), invalid.len()),
                    0
                );
            }
            assert_eq!(cubacadabra_app_dispatch_json(app, ptr::null(), 1), 0);
            for action in [
                r#"{"type":"replace_session","account_id":"a","username":"Ada"}"#,
                r#"{"type":"username_changed","value":"Grace"}"#,
                r#"{"type":"save_username"}"#,
            ] {
                assert_eq!(
                    cubacadabra_app_dispatch_json(app, action.as_ptr(), action.len()),
                    1
                );
            }
            assert_eq!(cubacadabra_app_poll_effect_json(app), 1);
            let effect: serde_json::Value = serde_json::from_slice(slice::from_raw_parts(
                cubacadabra_app_output_ptr(app),
                cubacadabra_app_output_len(app),
            ))
            .unwrap();
            assert_eq!(effect["path"], "auth/username");
            assert_eq!(effect["body"], r#"{"username":"Grace"}"#);
            assert_eq!(cubacadabra_app_poll_effect_json(app), 0);
            assert_eq!(cubacadabra_app_output_len(app), 0);
            assert_eq!(cubacadabra_app_snapshot_json(app), 1);
            let snapshot: serde_json::Value = serde_json::from_slice(slice::from_raw_parts(
                cubacadabra_app_output_ptr(app),
                cubacadabra_app_output_len(app),
            ))
            .unwrap();
            assert_eq!(snapshot["profile"]["username_is_saving"], true);
            cubacadabra_app_destroy(app);
        }
    }
}
