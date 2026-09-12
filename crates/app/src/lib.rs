mod appearance;
mod catalog;
#[cfg(not(target_arch = "wasm32"))]
mod ffi;
mod http;
mod profile;
mod safety;
mod username;
#[cfg(target_arch = "wasm32")]
mod web;

pub use appearance::{
    AppearanceFeedback, AppearanceSnapshot, MorphAssetSnapshot, MorphPresetSnapshot,
};
use catalog::CatalogState;
pub use catalog::{CatalogEntry, CatalogFeedback, CatalogFeedbackKind, CatalogSnapshot};
use profile::ProfileState;
pub use profile::{
    BirthdayFeedback, BirthdaySaveError, BodyFeedback, BodySaveError, DEFAULT_BODY_ID,
    FeedbackKind, PLAYER_BODY_IDS, ProfileSnapshot, UsernameFeedback, UsernameFeedbackCode,
    UsernameSaveError, is_valid_body_id, is_valid_date_of_birth,
};
pub use safety::{SafetyFeedback, SafetyFeedbackKind, SafetyPendingAction, SafetySnapshot};
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
    LoadAppearanceCatalog {},
    BeginAppearanceEdit {},
    SelectMorphPreset {
        preset_id: String,
    },
    SetMorphPart {
        asset_id: String,
    },
    ClearMorphPart {
        asset_id: String,
    },
    SaveAppearance {},
    SaveBirthday {
        date_of_birth: String,
    },
    LoadCatalog {
        #[serde(default = "default_catalog_page")]
        page: u16,
        #[serde(default = "default_catalog_page_size")]
        page_size: u16,
    },
    LoadBlockedUsers {},
    BlockUser {
        user_id: String,
    },
    UnblockUser {
        user_id: String,
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
    pub safety: SafetySnapshot,
    pub appearance: AppearanceSnapshot,
}

/// One model per host session, not per screen. Hosts render snapshots, execute
/// effects, and return raw responses here BEFORE projecting accepted state.
pub struct AppModel {
    account_id: Option<String>,
    session_id: u32,
    profile: ProfileState,
    catalog: CatalogState,
    safety: safety::SafetyState,
    appearance: appearance::AppearanceState,
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
            safety: safety::SafetyState::default(),
            appearance: appearance::AppearanceState::default(),
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
                self.safety.replace();
                self.appearance.replace();
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
            AppAction::LoadAppearanceCatalog {} => {
                let effect_id = EffectId(self.next_effect_id);
                if self.appearance.request_catalog(effect_id) {
                    self.next_effect_id += 1;
                    self.effects
                        .push_back(http::appearance_catalog_request(effect_id));
                }
            }
            AppAction::BeginAppearanceEdit {} => self.appearance.begin_edit(),
            AppAction::SelectMorphPreset { preset_id } => self.appearance.select_preset(&preset_id),
            AppAction::SetMorphPart { asset_id } => self.appearance.set_part(&asset_id),
            AppAction::ClearMorphPart { asset_id } => self.appearance.clear_part(&asset_id),
            AppAction::SaveAppearance {} => {
                if let Some(account_id) = self.account_id.clone() {
                    let effect_id = EffectId(self.next_effect_id);
                    if let Some((loadout, revision)) = self.appearance.request_save(effect_id) {
                        self.next_effect_id += 1;
                        self.effects.push_back(http::appearance_save_request(
                            effect_id, account_id, loadout, revision,
                        ));
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
            AppAction::LoadBlockedUsers {} => {
                if let Some(account_id) = self.account_id.clone() {
                    let effect_id = EffectId(self.next_effect_id);
                    if self.safety.request_load(effect_id) {
                        self.next_effect_id = self
                            .next_effect_id
                            .checked_add(1)
                            .expect("effect ID exhausted");
                        self.effects
                            .push_back(http::blocked_users_request(effect_id, account_id));
                    }
                }
            }
            AppAction::BlockUser { user_id } => {
                if let Some(account_id) = self.account_id.clone() {
                    let effect_id = EffectId(self.next_effect_id);
                    if let Some(user_id) = safety::normalize_requested_user_id(&user_id) {
                        if self.safety.request_block(effect_id, &user_id) {
                            self.next_effect_id = self
                                .next_effect_id
                                .checked_add(1)
                                .expect("effect ID exhausted");
                            self.effects.push_back(http::block_user_request(
                                effect_id, account_id, user_id,
                            ));
                        }
                    } else {
                        self.safety.invalid_request(
                            "invalid_block_target",
                            "That player could not be blocked.",
                        );
                    }
                }
            }
            AppAction::UnblockUser { user_id } => {
                if let Some(account_id) = self.account_id.clone() {
                    let effect_id = EffectId(self.next_effect_id);
                    if let Some(user_id) = safety::normalize_requested_user_id(&user_id) {
                        if self.safety.request_unblock(effect_id, &user_id) {
                            self.next_effect_id = self
                                .next_effect_id
                                .checked_add(1)
                                .expect("effect ID exhausted");
                            self.effects.push_back(http::unblock_user_request(
                                effect_id, account_id, user_id,
                            ));
                        }
                    } else {
                        self.safety.invalid_request(
                            "invalid_block_target",
                            "That player could not be unblocked.",
                        );
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
                } else if self.appearance.is_pending(effect_id) {
                    let request = self.appearance.pending_kind(effect_id);
                    let result = match request {
                        Some(appearance::PendingAppearanceRequest::Catalog(_)) => {
                            self.appearance.catalog_loaded(effect_id, &body)
                        }
                        Some(appearance::PendingAppearanceRequest::Load(_)) => {
                            self.appearance.appearance_loaded(effect_id, status, &body)
                        }
                        Some(appearance::PendingAppearanceRequest::Save(_)) => {
                            self.appearance.appearance_saved(effect_id, status, &body)
                        }
                        None => Err(()),
                    };
                    if result.is_err() {
                        self.appearance.failed(effect_id);
                    } else if matches!(
                        request,
                        Some(appearance::PendingAppearanceRequest::Catalog(_))
                    ) {
                        if let Some(account_id) = self.account_id.clone() {
                            let next_id = EffectId(self.next_effect_id);
                            self.next_effect_id += 1;
                            if self.appearance.request_load(next_id) {
                                self.effects
                                    .push_back(http::appearance_load_request(next_id, account_id));
                            }
                        }
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
                } else if self.safety.is_pending(effect_id) {
                    match self.safety.pending_action(effect_id) {
                        Some(safety::SafetyPendingAction::Load) => {
                            match safety::blocked_users_response(status, &body) {
                                Ok(ids) => self.safety.loaded(effect_id, ids),
                                Err((code, message)) => {
                                    self.safety.failed(effect_id, code, message)
                                }
                            }
                        }
                        Some(safety::SafetyPendingAction::Block) => {
                            match safety::action_response(
                                status,
                                &body,
                                "That player could not be blocked. Please try again.",
                            ) {
                                Ok(()) => self.safety.action_succeeded(effect_id),
                                Err((code, message)) => {
                                    self.safety.failed(effect_id, code, message)
                                }
                            }
                        }
                        Some(safety::SafetyPendingAction::Unblock) => {
                            match safety::action_response(
                                status,
                                &body,
                                "That player could not be unblocked. Please try again.",
                            ) {
                                Ok(()) => self.safety.action_succeeded(effect_id),
                                Err((code, message)) => {
                                    self.safety.failed(effect_id, code, message)
                                }
                            }
                        }
                        None => {}
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
                } else if self.appearance.is_pending(effect_id) {
                    self.appearance.failed(effect_id)
                } else if self.catalog.is_pending(effect_id) {
                    self.catalog.failed(
                        effect_id,
                        "unavailable",
                        "We couldn’t load the cubes. Please try again.",
                    )
                } else if self.safety.is_pending(effect_id) {
                    let (code, message) = match self.safety.pending_action(effect_id) {
                        Some(safety::SafetyPendingAction::Load) => (
                            "unavailable",
                            "We couldn’t load your blocked users. Please try again.",
                        ),
                        Some(safety::SafetyPendingAction::Block) => (
                            "unavailable",
                            "That player could not be blocked. Please try again.",
                        ),
                        Some(safety::SafetyPendingAction::Unblock) => (
                            "unavailable",
                            "That player could not be unblocked. Please try again.",
                        ),
                        None => return,
                    };
                    self.safety.failed(effect_id, code, message);
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
            safety: self.safety.snapshot(),
            appearance: self.appearance.snapshot(),
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
