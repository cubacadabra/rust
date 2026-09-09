use crate::AppModel;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Default)]
pub struct WebApp {
    model: AppModel,
}

#[wasm_bindgen]
impl WebApp {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dispatch_json(&mut self, source: &str) -> Result<(), JsValue> {
        self.model
            .dispatch_json(source)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&self.model.snapshot()).expect("app snapshots are serializable")
    }

    pub fn poll_effect_json(&mut self) -> Option<String> {
        self.model
            .poll_effect()
            .map(|effect| serde_json::to_string(&effect).expect("app effects are serializable"))
    }
}
