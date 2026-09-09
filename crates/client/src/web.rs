use wasm_bindgen::prelude::*;

use crate::{ClientAction, ClientSession};

#[wasm_bindgen]
pub struct WebClient {
    session: ClientSession,
}

#[wasm_bindgen]
impl WebClient {
    #[wasm_bindgen(constructor)]
    pub fn new(manifest: &str, script: &str) -> Result<WebClient, JsValue> {
        ClientSession::load(manifest, script)
            .map(|session| Self { session })
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    pub fn engine_handle(&mut self) -> usize {
        self.session.engine_mut() as *mut _ as usize
    }

    pub fn transport_connected(&mut self) {
        self.session.transport_connected();
    }

    pub fn transport_disconnected(&mut self) {
        self.session.transport_disconnected();
    }

    pub fn request_transport(&mut self) {
        self.session.request_transport();
    }

    pub fn receive_text(&mut self, source: &str) -> bool {
        self.session.receive_text(source)
    }

    pub fn set_ignored_player_ids_json(&mut self, source: &str) -> bool {
        let Ok(player_ids) = serde_json::from_str::<Vec<String>>(source) else {
            return false;
        };
        self.session.set_ignored_player_ids(player_ids);
        true
    }

    pub fn poll_actions_json(&mut self) -> String {
        let actions = self
            .session
            .poll_actions()
            .into_iter()
            .map(|action| match action {
                ClientAction::SetWorld(world_id) => {
                    serde_json::json!({ "type": "set_world", "worldId": world_id })
                }
                ClientAction::SendText(source) => {
                    serde_json::json!({ "type": "send_text", "source": source })
                }
            })
            .collect::<Vec<_>>();
        serde_json::to_string(&actions).unwrap_or_else(|_| "[]".to_owned())
    }
}
