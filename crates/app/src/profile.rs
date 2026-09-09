use serde::{Deserialize, Serialize};

use crate::UsernameValidationError;
use crate::{AppEffect, EffectId, validate_account_username};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsernameSaveError {
    Taken,
    NotAllowed,
    AgeRequired,
    Unauthorized,
    Unavailable,
    InvalidResponse,
}

impl UsernameSaveError {
    pub fn from_server_code(code: &str) -> Self {
        match code {
            "username_taken" => Self::Taken,
            "username_not_allowed" | "invalid_username" => Self::NotAllowed,
            "age_required" => Self::AgeRequired,
            "not_authenticated" | "unauthorized" => Self::Unauthorized,
            _ => Self::Unavailable,
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::Taken => "That username is already in use. Try another.",
            Self::NotAllowed => "That username isn’t available. Try another.",
            Self::AgeRequired => "Complete the birthday step before choosing a username.",
            Self::Unauthorized => "Your sign-in has expired. Please sign in again.",
            Self::Unavailable | Self::InvalidResponse => {
                "We couldn’t save your username. Please try again."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    Success,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UsernameFeedbackCode {
    Validation { error: UsernameValidationError },
    Saved,
    SaveFailed { error: UsernameSaveError },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsernameFeedback {
    pub kind: FeedbackKind,
    pub code: UsernameFeedbackCode,
    pub message: String,
}

impl UsernameFeedback {
    fn validation(error: UsernameValidationError) -> Self {
        Self {
            kind: FeedbackKind::Error,
            code: UsernameFeedbackCode::Validation { error },
            message: error.message().to_owned(),
        }
    }

    fn saved() -> Self {
        Self {
            kind: FeedbackKind::Success,
            code: UsernameFeedbackCode::Saved,
            message: "Username saved.".to_owned(),
        }
    }

    fn save_failed(error: UsernameSaveError) -> Self {
        Self {
            kind: FeedbackKind::Error,
            code: UsernameFeedbackCode::SaveFailed { error },
            message: error.message().to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSnapshot {
    pub username: Option<String>,
    pub username_draft: String,
    pub username_validation_error: Option<UsernameValidationError>,
    pub username_is_dirty: bool,
    pub username_can_save: bool,
    pub username_is_saving: bool,
    pub username_feedback: Option<UsernameFeedback>,
}

#[derive(Debug)]
struct PendingUsernameSave {
    effect_id: EffectId,
    username: String,
}

#[derive(Debug)]
pub(crate) struct ProfileState {
    username: Option<String>,
    username_draft: String,
    pending_username_save: Option<PendingUsernameSave>,
    username_feedback: Option<UsernameFeedback>,
}

impl ProfileState {
    pub(crate) fn new(username: Option<String>) -> Self {
        let username = clean_current_username(username);
        Self {
            username_draft: username.clone().unwrap_or_default(),
            username,
            pending_username_save: None,
            username_feedback: None,
        }
    }

    pub(crate) fn replace(&mut self, username: Option<String>) {
        *self = Self::new(username);
    }

    pub(crate) fn change_username(&mut self, value: String) {
        self.username_draft = value;
        self.username_feedback = None;
    }

    pub(crate) fn request_username_save(&mut self, effect_id: EffectId) -> Option<AppEffect> {
        if self.pending_username_save.is_some() {
            return None;
        }
        let username = match validate_account_username(&self.username_draft) {
            Ok(username) => username,
            Err(error) => {
                self.username_feedback = Some(UsernameFeedback::validation(error));
                return None;
            }
        };
        if self.username.as_deref() == Some(username.as_str()) {
            return None;
        }
        self.username_feedback = None;
        self.pending_username_save = Some(PendingUsernameSave {
            effect_id,
            username: username.clone(),
        });
        Some(AppEffect::SaveUsername {
            effect_id,
            username,
        })
    }

    pub(crate) fn username_saved(&mut self, effect_id: EffectId, username: String) {
        let Some(pending) = self.take_matching_save(effect_id) else {
            return;
        };
        let Ok(username) = validate_account_username(&username) else {
            self.username_feedback = Some(UsernameFeedback::save_failed(
                UsernameSaveError::InvalidResponse,
            ));
            return;
        };
        let draft_matches_submission = validate_account_username(&self.username_draft)
            .is_ok_and(|draft| draft == pending.username);
        self.username = Some(username.clone());
        if draft_matches_submission {
            self.username_draft = username;
        }
        self.username_feedback = Some(UsernameFeedback::saved());
    }

    pub(crate) fn username_save_failed(&mut self, effect_id: EffectId, error: UsernameSaveError) {
        if self.take_matching_save(effect_id).is_some() {
            self.username_feedback = Some(UsernameFeedback::save_failed(error));
        }
    }

    pub(crate) fn clear_username_feedback(&mut self) {
        self.username_feedback = None;
    }

    pub(crate) fn snapshot(&self) -> ProfileSnapshot {
        let validation = validate_account_username(&self.username_draft);
        let normalized = validation.as_ref().ok();
        let validation_error = validation.as_ref().err().copied();
        let trimmed_draft = self.username_draft.trim();
        let draft_value = (!trimmed_draft.is_empty()).then_some(trimmed_draft);
        let is_dirty = self.username.as_deref() != draft_value;
        ProfileSnapshot {
            username: self.username.clone(),
            username_draft: self.username_draft.clone(),
            username_validation_error: validation_error,
            username_is_dirty: is_dirty,
            username_can_save: self.pending_username_save.is_none()
                && normalized.is_some()
                && is_dirty,
            username_is_saving: self.pending_username_save.is_some(),
            username_feedback: self.username_feedback.clone(),
        }
    }

    fn take_matching_save(&mut self, effect_id: EffectId) -> Option<PendingUsernameSave> {
        if self
            .pending_username_save
            .as_ref()
            .is_none_or(|pending| pending.effect_id != effect_id)
        {
            return None;
        }
        self.pending_username_save.take()
    }
}

fn clean_current_username(username: Option<String>) -> Option<String> {
    username.and_then(|username| {
        let username = username.trim();
        (!username.is_empty()).then(|| username.to_owned())
    })
}
