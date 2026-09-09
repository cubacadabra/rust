use std::{collections::VecDeque, ptr, slice};

use cubacadabra_engine::Engine;

use crate::{ClientAction, ClientSession};

pub struct CubacadabraClient {
    session: ClientSession,
    pending_actions: VecDeque<ClientAction>,
    action_buffer: Vec<u8>,
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_create(
    manifest: *const u8,
    manifest_length: usize,
    script: *const u8,
    script_length: usize,
) -> *mut CubacadabraClient {
    let Some(manifest) = (unsafe { source(manifest, manifest_length) }) else {
        return ptr::null_mut();
    };
    let Some(script) = (unsafe { source(script, script_length) }) else {
        return ptr::null_mut();
    };
    let Ok(session) = ClientSession::load(manifest, script) else {
        return ptr::null_mut();
    };
    Box::into_raw(Box::new(CubacadabraClient {
        session,
        pending_actions: VecDeque::new(),
        action_buffer: Vec::new(),
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_destroy(client: *mut CubacadabraClient) {
    if !client.is_null() {
        drop(unsafe { Box::from_raw(client) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_engine(client: *mut CubacadabraClient) -> *mut Engine {
    unsafe { client.as_mut() }
        .map(|client| client.session.engine_mut() as *mut Engine)
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_transport_connected(client: *mut CubacadabraClient) {
    if let Some(client) = unsafe { client.as_mut() } {
        client.session.transport_connected();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_transport_disconnected(client: *mut CubacadabraClient) {
    if let Some(client) = unsafe { client.as_mut() } {
        client.session.transport_disconnected();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_request_transport(client: *mut CubacadabraClient) {
    if let Some(client) = unsafe { client.as_mut() } {
        client.session.request_transport();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_receive_text(
    client: *mut CubacadabraClient,
    message: *const u8,
    message_length: usize,
) -> u8 {
    let Some(client) = (unsafe { client.as_mut() }) else {
        return 0;
    };
    let Some(message) = (unsafe { source(message, message_length) }) else {
        return 0;
    };
    u8::from(client.session.receive_text(message))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_set_ignored_player_ids_json(
    client: *mut CubacadabraClient,
    player_ids: *const u8,
    player_ids_length: usize,
) -> u8 {
    let Some(client) = (unsafe { client.as_mut() }) else {
        return 0;
    };
    let Some(player_ids) = (unsafe { source(player_ids, player_ids_length) }) else {
        return 0;
    };
    let Ok(player_ids) = serde_json::from_str::<Vec<String>>(player_ids) else {
        return 0;
    };
    client.session.set_ignored_player_ids(player_ids);
    1
}

/// Returns 0 for no work, 1 for SetWorld, and 2 for SendText.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_poll_action(client: *mut CubacadabraClient) -> u8 {
    let Some(client) = (unsafe { client.as_mut() }) else {
        return 0;
    };
    if client.pending_actions.is_empty() {
        client.pending_actions.extend(client.session.poll_actions());
    }
    let Some(action) = client.pending_actions.pop_front() else {
        client.action_buffer.clear();
        return 0;
    };
    let (kind, source) = match action {
        ClientAction::SetWorld(world_id) => (1, world_id),
        ClientAction::SendText(source) => (2, source),
    };
    client.action_buffer = source.into_bytes();
    kind
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_action_ptr(client: *const CubacadabraClient) -> *const u8 {
    unsafe { client.as_ref() }
        .map(|client| client.action_buffer.as_ptr())
        .unwrap_or(ptr::null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn client_action_len(client: *const CubacadabraClient) -> usize {
    unsafe { client.as_ref() }
        .map(|client| client.action_buffer.len())
        .unwrap_or(0)
}
