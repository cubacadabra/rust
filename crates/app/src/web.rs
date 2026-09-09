use std::collections::VecDeque;

use wasm_bindgen::prelude::*;

use crate::{AppAction, AppEffect, AppModel, EffectId, UsernameSaveError};

#[wasm_bindgen]
pub struct WebApp {
    model: AppModel,
    pending_effects: VecDeque<AppEffect>,
}

#[wasm_bindgen]
impl WebApp {
    #[wasm_bindgen(constructor)]
    pub fn new(username: Option<String>) -> Self {
        Self {
            model: AppModel::new(username),
            pending_effects: VecDeque::new(),
        }
    }

    pub fn replace_profile(&mut self, username: Option<String>) {
        self.model.dispatch(AppAction::ReplaceProfile { username });
        self.pending_effects.clear();
    }

    pub fn username_changed(&mut self, value: String) {
        self.model.dispatch(AppAction::UsernameChanged { value });
    }

    pub fn save_username(&mut self) {
        self.model.dispatch(AppAction::SaveUsername);
    }

    pub fn username_saved(&mut self, effect_id: u32, username: String) {
        self.model.dispatch(AppAction::UsernameSaved {
            effect_id: EffectId(effect_id),
            username,
        });
    }

    pub fn username_save_failed(&mut self, effect_id: u32, server_code: &str) {
        self.model.dispatch(AppAction::UsernameSaveFailed {
            effect_id: EffectId(effect_id),
            error: UsernameSaveError::from_server_code(server_code),
        });
    }

    pub fn clear_username_feedback(&mut self) {
        self.model.dispatch(AppAction::ClearUsernameFeedback);
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&self.model.snapshot()).expect("app snapshots are serializable")
    }

    pub fn poll_effect_json(&mut self) -> Option<String> {
        if self.pending_effects.is_empty() {
            self.pending_effects.extend(self.model.take_effects());
        }
        self.pending_effects
            .pop_front()
            .map(|effect| serde_json::to_string(&effect).expect("app effects are serializable"))
    }
}
