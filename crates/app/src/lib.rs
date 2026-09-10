mod catalog;
#[cfg(not(target_arch = "wasm32"))]
mod ffi;
mod http;
mod profile;
mod username;
#[cfg(target_arch = "wasm32")]
mod web;

use catalog::CatalogState;
pub use catalog::{CatalogEntry, CatalogFeedback, CatalogFeedbackKind, CatalogSnapshot};
use profile::ProfileState;
pub use profile::{
    is_valid_body_id, is_valid_date_of_birth, BirthdayFeedback, BirthdaySaveError, BodyFeedback,
    BodySaveError, FeedbackKind, ProfileSnapshot, UsernameFeedback, UsernameFeedbackCode,
    UsernameSaveError, DEFAULT_BODY_ID, PLAYER_BODY_IDS,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
pub use username::{
    validate_account_username, UsernameValidationError, USERNAME_MAX_CHARACTERS,
    USERNAME_MIN_CHARACTERS,
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
        #[serde(default)]
        body_id: Option<String>,
        #[serde(default)]
        date_of_birth: Option<String>,
    },
    BeginUsernameEdit {},
    UsernameChanged {
        value: String,
    },
    SaveUsername {},
    BeginBodyEdit {},
    BodyChanged {
        body_id: String,
    },
    SaveBody {},
    SaveBirthday {
        date_of_birth: String,
    },
    LoadCatalog {
        #[serde(default = "default_catalog_page")]
        page: u16,
        #[serde(default = "default_catalog_page_size")]
        page_size: u16,
    },
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
    /// Relative to the host's configured API origin. `None` marks a public
    /// request; credentials never enter Rust.
    HttpRequest {
        effect_id: EffectId,
        account_id: Option<String>,
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
    pub catalog: CatalogSnapshot,
}

/// One model per host session, not per screen. Hosts render snapshots, execute
/// effects, and return raw responses here BEFORE projecting accepted state.
pub struct AppModel {
    account_id: Option<String>,
    session_id: u32,
    profile: ProfileState,
    catalog: CatalogState,
    effects: VecDeque<AppEffect>,
    next_effect_id: u32,
}

impl Default for AppModel {
    fn default() -> Self {
        Self {
            account_id: None,
            session_id: 0,
            profile: ProfileState::new(None, None, None),
            catalog: CatalogState::default(),
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
                body_id,
                date_of_birth,
            } => {
                self.account_id = account_id.filter(|id| !id.is_empty());
                self.session_id = self
                    .session_id
                    .checked_add(1)
                    .expect("session ID exhausted");
                let body_id = self
                    .account_id
                    .as_ref()
                    .and(body_id)
                    .or_else(|| self.account_id.as_ref().map(|_| DEFAULT_BODY_ID.to_owned()));
                self.profile.replace(
                    self.account_id.as_ref().and(username),
                    body_id,
                    date_of_birth,
                );
                self.catalog.replace();
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
            AppAction::BeginBodyEdit {} => self.profile.begin_body_edit(),
            AppAction::BodyChanged { body_id } => {
                if self.account_id.is_some() {
                    self.profile.change_body(body_id);
                }
            }
            AppAction::SaveBody {} => {
                if let Some(account_id) = self.account_id.clone() {
                    let effect_id = EffectId(self.next_effect_id);
                    if let Some(body_id) = self.profile.request_body_save(effect_id) {
                        self.next_effect_id = self
                            .next_effect_id
                            .checked_add(1)
                            .expect("effect ID exhausted");
                        self.effects
                            .push_back(http::body_request(effect_id, account_id, body_id));
                    }
                }
            }
            AppAction::SaveBirthday { date_of_birth } => {
                if let Some(account_id) = self.account_id.clone() {
                    let effect_id = EffectId(self.next_effect_id);
                    if let Some(date_of_birth) =
                        self.profile.request_birthday_save(effect_id, date_of_birth)
                    {
                        self.next_effect_id = self
                            .next_effect_id
                            .checked_add(1)
                            .expect("effect ID exhausted");
                        self.effects.push_back(http::birthday_request(
                            effect_id,
                            account_id,
                            date_of_birth,
                        ));
                    }
                }
            }
            AppAction::LoadCatalog { page, page_size } => {
                let effect_id = EffectId(self.next_effect_id);
                if let Some((page, page_size)) =
                    self.catalog.request_load(effect_id, page, page_size)
                {
                    self.next_effect_id = self
                        .next_effect_id
                        .checked_add(1)
                        .expect("effect ID exhausted");
                    self.effects.push_back(http::catalog_request(
                        effect_id,
                        self.account_id.clone(),
                        page,
                        page_size,
                    ));
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
                } else if self.profile.is_body_pending(effect_id) {
                    let expected = self.profile.pending_body_id(effect_id).unwrap_or_default();
                    match http::body_response(status, &body, self.account_id.as_deref(), expected) {
                        Ok(body_id) => self.profile.body_saved(effect_id, body_id),
                        Err(error) => self.profile.body_save_failed(effect_id, error),
                    }
                } else if self.profile.is_birthday_pending(effect_id) {
                    let expected = self
                        .profile
                        .pending_birthday_date_of_birth(effect_id)
                        .unwrap_or_default();
                    match http::birthday_response(
                        status,
                        &body,
                        self.account_id.as_deref(),
                        expected,
                    ) {
                        Ok(date_of_birth) => self.profile.birthday_saved(effect_id, date_of_birth),
                        Err(error) => self.profile.birthday_save_failed(effect_id, error),
                    }
                } else if self.catalog.is_pending(effect_id) {
                    match catalog::response(status, &body) {
                        Ok(page) => self.catalog.loaded(
                            effect_id,
                            page.page,
                            page.entries,
                            page.has_next_page,
                        ),
                        Err((code, message)) => self.catalog.failed(effect_id, code, message),
                    }
                }
            }
            AppAction::HttpFailed { effect_id } => {
                if self.profile.is_pending(effect_id) {
                    self.profile
                        .username_save_failed(effect_id, UsernameSaveError::Unavailable)
                } else if self.profile.is_body_pending(effect_id) {
                    self.profile
                        .body_save_failed(effect_id, BodySaveError::Unavailable)
                } else if self.profile.is_birthday_pending(effect_id) {
                    self.profile
                        .birthday_save_failed(effect_id, BirthdaySaveError::Unavailable)
                } else if self.catalog.is_pending(effect_id) {
                    self.catalog.failed(
                        effect_id,
                        "unavailable",
                        "We couldn’t load the cubes. Please try again.",
                    )
                }
            }
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
            catalog: self.catalog.snapshot(),
        }
    }

    pub fn poll_effect(&mut self) -> Option<AppEffect> {
        self.effects.pop_front()
    }
    pub fn take_effects(&mut self) -> Vec<AppEffect> {
        self.effects.drain(..).collect()
    }
}

fn default_catalog_page_size() -> u16 {
    catalog::DEFAULT_CATALOG_PAGE_SIZE
}

fn default_catalog_page() -> u16 {
    1
}

#[cfg(test)]
mod tests;
