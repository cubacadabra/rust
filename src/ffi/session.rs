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
pub unsafe extern "C" fn engine_script_error_ptr(engine: *const Engine) -> *const u8 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.script_error_buffer().as_ptr())
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_script_error_len(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }
        .map(|engine| engine.script_error_buffer().len())
        .unwrap_or(0)
}

/// Queues one host-received game message for the next Luau tick. The engine
/// treats the message as opaque JSON; the game package owns its schema.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create` and
/// `source` must be null only when `length` is zero or otherwise point to
/// `length` readable bytes for the duration of this call.
pub unsafe extern "C" fn engine_receive_network_message_json(
    engine: *mut Engine,
    source: *const u8,
    length: usize,
) -> u8 {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return 0;
    };
    if source.is_null() && length != 0 {
        return 0;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(source, length) }
    };
    let Ok(source) = std::str::from_utf8(bytes) else {
        return 0;
    };
    u8::from(engine.receive_network_message_json(source))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`; the
/// returned buffer must be written with exactly `length` bytes before load.
pub unsafe extern "C" fn engine_network_receive_buffer_ptr(
    engine: *mut Engine,
    length: usize,
) -> *mut u8 {
    unsafe { engine.as_mut() }
        .map(|engine| engine.prepare_network_receive_buffer(length))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_load_network_receive_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.load_network_receive_buffer()))
        .unwrap_or(0)
}

/// Polls one outbound game message emitted by Luau. The pointer remains valid
/// until the next poll or engine destruction.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_network_poll_message(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.poll_network_message()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_network_message_ptr(engine: *const Engine) -> *const u8 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.network_message().as_ptr())
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_network_message_len(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.network_message().len())
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
pub unsafe extern "C" fn engine_username_buffer_ptr(engine: *mut Engine, length: usize) -> *mut u8 {
    unsafe { engine.as_mut() }.map_or(ptr::null_mut(), |engine| {
        engine.prepare_username_buffer(length)
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_load_username_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }.map_or(0, |engine| u8::from(engine.load_username_buffer()))
}

/// Returns a bounded engine-owned UTF-8 input buffer for a versioned local
/// character appearance. The pointer is valid until the next appearance
/// buffer allocation or engine destruction.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`; the
/// returned buffer must be written with exactly `length` bytes before load.
pub unsafe extern "C" fn engine_appearance_buffer_ptr(
    engine: *mut Engine,
    length: usize,
) -> *mut u8 {
    unsafe { engine.as_mut() }
        .map(|engine| engine.prepare_appearance_buffer(length))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_load_appearance_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.load_appearance_buffer()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_morph_loadout_buffer_ptr(
    engine: *mut Engine,
    length: usize,
) -> *mut u8 {
    unsafe { engine.as_mut() }
        .map(|engine| engine.prepare_appearance_buffer(length))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_load_morph_loadout_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.load_morph_loadout_buffer()))
        .unwrap_or(0)
}

/// Applies a local appearance supplied as a borrowed UTF-8 JSON span. This is
/// a convenience for hosts that do not need the persistent engine buffer.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create` and
/// `source` must be null only when `length` is zero or otherwise point to
/// `length` readable bytes for the duration of this call.
pub unsafe extern "C" fn engine_set_local_appearance_json(
    engine: *mut Engine,
    source: *const u8,
    length: usize,
) -> u8 {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return 0;
    };
    if source.is_null() && length != 0 {
        return 0;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(source, length) }
    };
    let Ok(source) = std::str::from_utf8(bytes) else {
        engine.appearance_status = 0;
        return 0;
    };
    engine.set_local_appearance_json(source)
}

#[unsafe(no_mangle)]
/// Applies a schema-2 morph loadout directly, without projecting through the
/// legacy body/outfit/equipment representation.
///
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create` and
/// `source` must be null only when `length` is zero or otherwise point to
/// `length` readable bytes for the duration of this call.
pub unsafe extern "C" fn engine_set_local_morph_loadout_json(
    engine: *mut Engine,
    source: *const u8,
    length: usize,
) -> u8 {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return 0;
    };
    if source.is_null() && length != 0 {
        return 0;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(source, length) }
    };
    let Ok(source) = std::str::from_utf8(bytes) else {
        return 0;
    };
    engine.set_local_morph_loadout_json(source)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_appearance_status(engine: *const Engine) -> u8 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.appearance_status())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_appearance_revision(engine: *const Engine) -> u32 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.appearance_revision())
        .unwrap_or(0)
}

/// Returns a bounded engine-owned UTF-8 input buffer for one versioned remote
/// roster update. The message is applied atomically when it passes validation.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`; the
/// returned buffer must be written with exactly `length` bytes before load.
pub unsafe extern "C" fn engine_remote_update_buffer_ptr(
    engine: *mut Engine,
    length: usize,
) -> *mut u8 {
    unsafe { engine.as_mut() }
        .map(|engine| engine.prepare_remote_update_buffer(length))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_apply_remote_update_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.apply_remote_update_buffer()))
        .unwrap_or(0)
}

/// Returns a bounded engine-owned byte buffer for one compact remote-motion
/// batch. The buffer is applied atomically by the matching load function.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_remote_motion_batch_buffer_ptr(
    engine: *mut Engine,
    length: usize,
) -> *mut u8 {
    unsafe { engine.as_mut() }
        .map(|engine| engine.prepare_remote_motion_batch_buffer(length))
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_apply_remote_motion_batch_buffer(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.apply_remote_motion_batch_buffer()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create` and
/// `source` must be null only when `length` is zero or otherwise point to
/// `length` readable bytes for the duration of this call.
pub unsafe extern "C" fn engine_apply_remote_update_json(
    engine: *mut Engine,
    source: *const u8,
    length: usize,
) -> u8 {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return 0;
    };
    if source.is_null() && length != 0 {
        return 0;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(source, length) }
    };
    let Ok(source) = std::str::from_utf8(bytes) else {
        engine.remote_update_status = 0;
        return 0;
    };
    u8::from(engine.apply_remote_update_json(source))
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_remote_update_status(engine: *const Engine) -> u8 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.remote_update_status())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_remote_update_sequence(engine: *const Engine) -> u64 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.remote_update_sequence())
        .unwrap_or(0)
}

/// Starts a new remote roster lifetime. Cached appearances remain bounded and
/// may hydrate a reconnect that omits unchanged content; motion and packet
/// sequencing are reset.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_reset_remote_session(engine: *mut Engine) {
    if let Some(engine) = unsafe { engine.as_mut() } {
        engine.reset_remote_session();
    }
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
pub unsafe extern "C" fn engine_player_facing_yaw(engine: *const Engine) -> f32 {
    unsafe { engine.as_ref() }.map_or(0.0, Engine::player_facing_yaw)
}

#[unsafe(no_mangle)]
/// Increments whenever falling respawns the local player at a checkpoint.
/// Hosts replicate this with movement so the server can authorize the teleport.
///
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_player_respawn_event_id(engine: *const Engine) -> u32 {
    unsafe { engine.as_ref() }.map_or(0, Engine::player_respawn_event_id)
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
pub unsafe extern "C" fn engine_settings_room_state(engine: *const Engine) -> u8 {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.settings_room_state())
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

/// Uploads the package-owned world image atlas and its normalized image
/// regions. The host owns both input buffers for the duration of this call.
#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
/// # Safety
/// `renderer` must be null or a live pointer returned by
/// `engine_renderer_create`. When non-zero, `pixels` must reference
/// `pixel_len` bytes and `regions` must reference `regions_len` UTF-8 bytes
/// for the duration of this call.
pub unsafe extern "C" fn engine_renderer_set_package_image_atlas(
    renderer: *mut Renderer,
    width: u32,
    height: u32,
    pixels: *const u8,
    pixel_len: usize,
    regions: *const u8,
    regions_len: usize,
) -> u8 {
    let Some(renderer) = (unsafe { renderer.as_mut() }) else {
        return 0;
    };
    if (pixel_len > 0 && pixels.is_null()) || (regions_len > 0 && regions.is_null()) {
        return 0;
    }
    let pixels = unsafe { slice::from_raw_parts(pixels, pixel_len) };
    let regions = unsafe { slice::from_raw_parts(regions, regions_len) };
    let Ok(regions) = std::str::from_utf8(regions) else {
        return 0;
    };
    let Ok(regions) = serde_json::from_str::<std::collections::BTreeMap<String, [f32; 4]>>(regions)
    else {
        return 0;
    };
    u8::from(renderer.set_package_image_atlas(width, height, pixels, regions))
}

/// Registers a validated compiled morph pack with the renderer. The pack is
/// decoded and uploaded as immutable GPU mesh resources; it is not parsed as
/// GLB or Blender data by the runtime.
#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
/// # Safety
/// `renderer` must be null or a live pointer returned by
/// `engine_renderer_create`. When non-zero, `bytes` must reference `length`
/// readable bytes for the duration of this call.
pub unsafe extern "C" fn engine_renderer_register_morph_pack(
    renderer: *mut Renderer,
    bytes: *const u8,
    length: usize,
) -> u8 {
    let Some(renderer) = (unsafe { renderer.as_mut() }) else {
        return 0;
    };
    if length > cubacadabra_morphs::MAX_MORPH_PACK_BYTES
        || (length > 0 && bytes.is_null())
    {
        return 0;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(bytes, length) }
    };
    u8::from(renderer.register_morph_pack(bytes).is_ok())
}

/// Selects the reversible character renderer rollout mode. `0` is the
/// legacy hard-cuboid renderer and `1` is the magic instanced renderer. An
/// invalid value leaves the current mode unchanged and returns zero.
#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
/// # Safety
/// `renderer` must be null or a live pointer returned by
/// `engine_renderer_create`.
pub unsafe extern "C" fn engine_renderer_set_appearance_mode(
    renderer: *mut Renderer,
    mode: u8,
) -> u8 {
    let Some(mode) = super::renderer::CharacterRenderMode::from_u8(mode) else {
        return 0;
    };
    let Some(renderer) = (unsafe { renderer.as_mut() }) else {
        return 0;
    };
    renderer.set_character_render_mode(mode);
    1
}

#[unsafe(no_mangle)]
#[cfg(not(target_arch = "wasm32"))]
/// # Safety
/// `renderer` must be null or a live pointer returned by
/// `engine_renderer_create`.
pub unsafe extern "C" fn engine_renderer_appearance_mode(renderer: *const Renderer) -> u8 {
    unsafe { renderer.as_ref() }
        .map(|renderer| renderer.character_render_mode().as_u8())
        .unwrap_or(super::renderer::CharacterRenderMode::Magic.as_u8())
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
/// # Safety
/// `renderer` must be null or a live pointer returned by
/// `engine_renderer_create`.
pub unsafe extern "C" fn engine_renderer_set_avatar_preview_mode(
    renderer: *mut Renderer,
    enabled: u8,
) -> u8 {
    let Some(renderer) = (unsafe { renderer.as_mut() }) else {
        return 0;
    };
    renderer.set_avatar_preview_mode(enabled != 0);
    1
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
