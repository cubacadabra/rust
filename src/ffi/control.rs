
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

/// Toggles cosmetic secondary effects. This does not change simulation,
/// collision, snapshots, or the core character silhouette.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_reduced_effects(engine: *mut Engine, reduced: u8) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_reduced_effects(reduced != 0);
    }
}

/// Triggers one presentation-only wave on the local character. The sequence
/// is edge-triggered by the renderer and does not affect simulation or the
/// multiplayer movement ABI.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_trigger_local_wave(engine: *mut Engine) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.trigger_local_wave();
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_ui_viewport(
    engine: *mut Engine,
    width: f32,
    height: f32,
    scale: f32,
    safe_top: f32,
    safe_right: f32,
    safe_bottom: f32,
    safe_left: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_ui_viewport(UiViewport {
            width,
            height,
            scale,
            safe_area: UiInsets {
                top: safe_top,
                right: safe_right,
                bottom: safe_bottom,
                left: safe_left,
            },
        });
    }
}

/// Sets the authentication state used by the shared logo modal. The platform
/// host owns authentication and must provide its own session state here.
///
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_set_authenticated(engine: *mut Engine, authenticated: u8) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_authenticated(authenticated != 0);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_ui_document_buffer_ptr(
    engine: *mut Engine,
    length: usize,
) -> *mut u8 {
    unsafe { engine.as_mut() }
        .map(|engine| engine.prepare_ui_document_buffer(length))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_load_ui_document_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.load_ui_document_buffer()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// Sends pointer coordinates in logical viewport units. Phase values are
/// 0=down, 1=move, 2=up, and 3=cancel.
///
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_ui_pointer(
    engine: *mut Engine,
    pointer_id: u64,
    phase: u8,
    x: f32,
    y: f32,
) -> u8 {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return 0;
    };
    let phase = match phase {
        0 => UiPointerPhase::Down,
        1 => UiPointerPhase::Move,
        2 => UiPointerPhase::Up,
        3 => UiPointerPhase::Cancel,
        _ => return 0,
    };
    u8::from(engine.ui_pointer(pointer_id, phase, x, y))
}

/// Returns whether the UI at the supplied logical viewport coordinates is
/// interactive. This is used by browser clients for hover affordances.
///
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_ui_hit_test(engine: *mut Engine, x: f32, y: f32) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.ui_hit_test(x, y)))
        .unwrap_or(0)
}

/// Returns whether the supplied logical viewport coordinates are over the
/// shared About link. Browser clients use this during pointer-up so opening a
/// new tab remains within the browser's user-activation window.
///
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_ui_external_link_hit_test(
    engine: *mut Engine,
    x: f32,
    y: f32,
) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.ui_external_link_hit_test(x, y)))
        .unwrap_or(0)
}

/// Returns whether the engine-owned shared logo modal is visible, including
/// its opening and closing animation.
///
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_ui_shared_modal_visible(engine: *const Engine) -> u8 {
    unsafe { engine.as_ref() }
        .map(|engine| u8::from(engine.ui_shared_modal_visible()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// Advances the host-facing UI event queue. The event is exposed as UTF-8 JSON
/// through `engine_ui_event_ptr` and `engine_ui_event_len` until the next poll.
///
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_ui_poll_event(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.ui.borrow_mut().poll_event()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_ui_event_ptr(engine: *const Engine) -> *const u8 {
    unsafe { engine.as_ref() }.map_or(ptr::null(), |engine| {
        engine.ui.borrow().event_buffer().as_ptr()
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_ui_event_len(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.ui.borrow().event_buffer().len())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_ui_node_count(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, Engine::ui_node_count)
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
pub unsafe extern "C" fn engine_reconcile_player(
    engine: *mut Engine,
    x: f32,
    y: f32,
    z: f32,
    yaw: f32,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.reconcile_player([x, y, z], yaw);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_build_block_count(engine: *mut Engine, count: usize) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_build_block_count(count);
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_set_build_block(
    engine: *mut Engine,
    index: usize,
    x: f32,
    y: f32,
    z: f32,
    width: f32,
    height: f32,
    depth: f32,
    color: u32,
    rotation: u8,
) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.set_build_block(index, [x, y, z], [width, height, depth], color, rotation);
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
