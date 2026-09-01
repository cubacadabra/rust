#![allow(clippy::too_many_arguments)]

use super::engine::{Engine, MAX_AGENTS, SNAPSHOT_STRIDE};
use super::types::Input;

#[unsafe(no_mangle)]
pub extern "C" fn engine_create() -> *mut Engine {
    Box::into_raw(Box::new(Engine::new()))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_input(
    engine: *mut Engine,
    forward: f32,
    strafe: f32,
    sprint: u8,
    jump: u8,
    look_x: f32,
    look_y: f32,
    zoom_delta: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_input(Input {
            forward,
            strafe,
            sprint: sprint != 0,
            jump: jump != 0,
            look_x,
            look_y,
            zoom_delta,
        });
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_step(engine: *mut Engine, delta: f32) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.step(delta);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_reset_view(engine: *mut Engine) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.reset_view();
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_snapshot_ptr(engine: *const Engine) -> *const f32 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.snapshot().as_ptr())
        .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn engine_snapshot_len() -> usize {
    (MAX_AGENTS + 1) * SNAPSHOT_STRIDE
}

#[unsafe(no_mangle)]
pub extern "C" fn engine_snapshot_stride() -> usize {
    SNAPSHOT_STRIDE
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_camera_yaw(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.camera()[0])
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_camera_pitch(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.camera()[1])
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_camera_distance(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.camera()[2])
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_agent_count(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.agent_count())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_meeting_count(engine: *const Engine, index: usize) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.meeting_count(index))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_elapsed(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.elapsed())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`, and it
/// must not be used again after this call.
pub unsafe extern "C" fn engine_destroy(engine: *mut Engine) {
    if !engine.is_null() {
        unsafe { drop(Box::from_raw(engine)) };
    }
}
