/// Polls one game-owned audio command emitted by Luau. The returned pointer
/// remains valid until the next poll or engine destruction.
#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_audio_poll_message(engine: *mut Engine) -> u8 {
    unsafe { engine.as_mut() }
        .map(|engine| u8::from(engine.poll_audio_message()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_audio_message_ptr(engine: *const Engine) -> *const u8 {
    unsafe { engine.as_ref() }
        .map(|engine| engine.audio_message().as_ptr())
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
/// # Safety
/// `engine` must be null or a live pointer returned by `engine_create`.
pub unsafe extern "C" fn engine_audio_message_len(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.audio_message().len())
}
