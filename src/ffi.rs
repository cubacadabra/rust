#![allow(clippy::too_many_arguments)]

use super::engine::{Engine, MAX_AGENTS, SNAPSHOT_STRIDE};
#[cfg(not(target_arch = "wasm32"))]
use super::renderer::Renderer;
use super::types::Input;
#[cfg(not(target_arch = "wasm32"))]
use std::ffi::c_void;
use std::ptr;

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
pub unsafe extern "C" fn engine_set_launch_pad(
    engine: *mut Engine,
    index: usize,
    x: f32,
    z: f32,
    radius: f32,
    countdown: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_launch_pad(index, x, z, radius, countdown);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_launch_pad_count(engine: *mut Engine, count: usize) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_launch_pad_count(count);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_obstacle(
    engine: *mut Engine,
    index: usize,
    x: f32,
    y: f32,
    z: f32,
    width: f32,
    height: f32,
    depth: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_obstacle(index, [x, y, z], [width, height, depth]);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_obstacle_count(engine: *mut Engine, count: usize) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_obstacle_count(count);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_world_count(engine: *mut Engine, count: usize) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_world_count(count);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_world_spawn(
    engine: *mut Engine,
    world: usize,
    x: f32,
    y: f32,
    z: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_world_spawn(world, [x, y, z]);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_world_launch_pad_count(
    engine: *mut Engine,
    world: usize,
    count: usize,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_world_launch_pad_count(world, count);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_world_launch_pad(
    engine: *mut Engine,
    world: usize,
    index: usize,
    x: f32,
    z: f32,
    radius: f32,
    countdown: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_world_launch_pad(world, index, x, z, radius, countdown);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_world_launch_destination(
    engine: *mut Engine,
    world: usize,
    pad: usize,
    destination: i32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_world_launch_destination(world, pad, destination);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_world_obstacle_count(
    engine: *mut Engine,
    world: usize,
    count: usize,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_world_obstacle_count(world, count);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_world_obstacle(
    engine: *mut Engine,
    world: usize,
    index: usize,
    x: f32,
    y: f32,
    z: f32,
    width: f32,
    height: f32,
    depth: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_world_obstacle(world, index, [x, y, z], [width, height, depth]);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_start_world(engine: *mut Engine, world: usize) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.start_world(world)))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_enter_session(
    engine: *mut Engine,
    launch_pad_index: usize,
    spawn_x: f32,
    spawn_y: f32,
    spawn_z: f32,
) -> usize {
    unsafe { engine.as_mut() }
        .map(|engine| engine.enter_session(launch_pad_index, [spawn_x, spawn_y, spawn_z]))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_script_buffer_ptr(engine: *mut Engine, length: usize) -> *mut u8 {
    unsafe { engine.as_mut() }
        .map(|engine| engine.prepare_script_buffer(length))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_load_script_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.load_script_buffer()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_package_buffer_ptr(engine: *mut Engine, length: usize) -> *mut u8 {
    unsafe { engine.as_mut() }
        .map(|engine| engine.prepare_package_buffer(length))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_load_package_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.load_package_buffer()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_script_loaded(engine: *const Engine) -> u8 {
    unsafe { engine.as_ref() }
        .map(|engine| u8::from(engine.script_loaded()))
        .unwrap_or(0)
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
pub unsafe extern "C" fn engine_local_agent_count(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.local_agent_count())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_remote_player_count(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.remote_player_count())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_remote_player_count(engine: *mut Engine, count: usize) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_remote_player_count(count);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_remote_player(
    engine: *mut Engine,
    index: usize,
    x: f32,
    y: f32,
    z: f32,
    yaw: f32,
    moving: u8,
    sprinting: u8,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_remote_player(index, [x, y, z], yaw, moving != 0, sprinting != 0);
    }
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
pub unsafe extern "C" fn engine_launch_pad_count(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.launch_pad_count())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_launch_pad_occupants(engine: *const Engine, index: usize) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.launch_pad_occupants(index))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_launch_pad_seconds(engine: *const Engine, index: usize) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, |engine| engine.launch_pad_seconds(index))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_launch_pad_phase(engine: *const Engine, index: usize) -> u8 {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.launch_pad_phase(index))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_player_launch_pad(engine: *const Engine) -> i32 {
    unsafe { engine.as_ref() }.map_or(-1, |engine| engine.player_launch_pad())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_launch_event_id(engine: *const Engine) -> u32 {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.launch_event_id())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_last_launch_pad(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.last_launch_pad())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_last_launch_occupants(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.last_launch_occupants())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_active_world(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.active_world())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_world_event_id(engine: *const Engine) -> u32 {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.world_event_id())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_last_world_source_pad(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.last_world_source_pad())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_last_world_destination(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.last_world_destination())
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

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
pub extern "C" fn engine_renderer_create(
    layer: *mut c_void,
    width: f32,
    height: f32,
) -> *mut Renderer {
    Renderer::new(layer, width, height)
        .map(|renderer| Box::into_raw(Box::new(renderer)))
        .unwrap_or(ptr::null_mut())
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
/// # Safety
/// `renderer` must be null or a live pointer returned by `engine_renderer_create`.
pub unsafe extern "C" fn engine_renderer_resize(renderer: *mut Renderer, width: f32, height: f32) {
    if let Some(renderer) = unsafe { renderer.as_mut() } {
        renderer.resize(width, height);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
/// # Safety
/// `renderer` and `engine` must be null or live pointers returned by their
/// corresponding create functions.
pub unsafe extern "C" fn engine_renderer_sync(renderer: *mut Renderer, engine: *const Engine) {
    let (Some(renderer), Some(engine)) = (unsafe { renderer.as_mut() }, unsafe { engine.as_ref() })
    else {
        return;
    };
    renderer.sync_engine(engine);
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
/// # Safety
/// `renderer` must be null or a live pointer returned by `engine_renderer_create`.
pub unsafe extern "C" fn engine_renderer_draw(renderer: *mut Renderer) {
    if let Some(renderer) = unsafe { renderer.as_mut() } {
        renderer.draw();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
/// # Safety
/// `renderer` must be null or a live renderer pointer and must not be used again.
pub unsafe extern "C" fn engine_renderer_destroy(renderer: *mut Renderer) {
    if !renderer.is_null() {
        unsafe { drop(Box::from_raw(renderer)) };
    }
}
