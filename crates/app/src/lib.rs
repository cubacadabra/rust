#[cfg(not(target_arch = "wasm32"))]
mod ffi;
mod http;
mod profile;
mod username;
#[cfg(target_arch = "wasm32")]
mod web;

use profile::ProfileState;
pub use profile::{
    FeedbackKind, ProfileSnapshot, UsernameFeedback, UsernameFeedbackCode, UsernameSaveError,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
pub use username::{
    USERNAME_MAX_CHARACTERS, USERNAME_MIN_CHARACTERS, UsernameValidationError,
    validate_account_username,
};
#[cfg(target_arch = "wasm32")]
pub use web::WebApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AppAction {
    /// Always invalidates outstanding work, even when signing back into the same account.
    ReplaceSession {
        account_id: Option<String>,
        username: Option<String>,
    },
    BeginUsernameEdit {},
    UsernameChanged {
        value: String,
    },
    SaveUsername {},
    HttpCompleted {
        effect_id: EffectId,
        status: u16,
        body: String,
    },
    HttpFailed {
        effect_id: EffectId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEffect {
    /// Relative to the host's configured API origin. Credentials never enter Rust.
    HttpRequest {
        effect_id: EffectId,
        account_id: String,
        method: String,
        path: String,
        body: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub protocol_version: u8,
    pub session_id: u32,
    pub account_id: Option<String>,
    pub profile: ProfileSnapshot,
}

/// One model per host session, not per screen. Hosts render snapshots, execute
/// effects, and return raw responses here BEFORE projecting accepted state.
pub struct AppModel {
    account_id: Option<String>,
    session_id: u32,
    profile: ProfileState,
    effects: VecDeque<AppEffect>,
    next_effect_id: u32,
}

impl Default for AppModel {
    fn default() -> Self {
        Self {
            account_id: None,
            session_id: 0,
            profile: ProfileState::new(None),
            effects: VecDeque::new(),
            next_effect_id: 1,
        }
    }
}

impl AppModel {
    pub fn dispatch(&mut self, action: AppAction) {
        match action {
            AppAction::ReplaceSession {
                account_id,
                username,
            } => {
                self.account_id = account_id.filter(|id| !id.is_empty());
                self.session_id = self
                    .session_id
                    .checked_add(1)
                    .expect("session ID exhausted");
                self.profile.replace(self.account_id.as_ref().and(username));
                self.effects.clear();
            }
            AppAction::BeginUsernameEdit {} => self.profile.begin_username_edit(),
            AppAction::UsernameChanged { value } => {
                if self.account_id.is_some() {
                    self.profile.change_username(value);
                }
            }
            AppAction::SaveUsername {} => {
                if let Some(account_id) = self.account_id.clone() {
                    let effect_id = EffectId(self.next_effect_id);
                    if let Some(username) = self.profile.request_username_save(effect_id) {
                        // Never reuse IDs: an old response must not match a new session.
                        self.next_effect_id = self
                            .next_effect_id
                            .checked_add(1)
                            .expect("effect ID exhausted");
                        self.effects
                            .push_back(http::username_request(effect_id, account_id, username));
                    }
                }
            }
            AppAction::HttpCompleted {
                effect_id,
                status,
                body,
            } => {
                if self.profile.is_pending(effect_id) {
                    match http::username_response(status, &body, self.account_id.as_deref()) {
                        Ok(username) => self.profile.username_saved(effect_id, username),
                        Err(error) => self.profile.username_save_failed(effect_id, error),
                    }
                }
            }
            AppAction::HttpFailed { effect_id } => self
                .profile
                .username_save_failed(effect_id, UsernameSaveError::Unavailable),
        }
    }

    /// Outer adapters share the same JSON contract; Rust consumers use dispatch directly.
    pub fn dispatch_json(&mut self, source: &str) -> Result<(), serde_json::Error> {
        let action = serde_json::from_str(source)?;
        self.dispatch(action);
        Ok(())
    }

    pub fn snapshot(&self) -> AppSnapshot {
        let mut profile = self.profile.snapshot();
        profile.username_can_save &= self.account_id.is_some();
        AppSnapshot {
            protocol_version: 1,
            session_id: self.session_id,
            account_id: self.account_id.clone(),
            profile,
        }
    }

    pub fn poll_effect(&mut self) -> Option<AppEffect> {
        self.effects.pop_front()
    }
    pub fn take_effects(&mut self) -> Vec<AppEffect> {
        self.effects.drain(..).collect()
    }
}

#[cfg(test)]
mod tests;
