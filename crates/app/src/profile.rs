use serde::{Deserialize, Serialize};

use crate::UsernameValidationError;
use crate::{validate_account_username, EffectId};

pub const DEFAULT_BODY_ID: &str = "cuba:person.v1";
pub const PLAYER_BODY_IDS: [&str; 3] =
    [DEFAULT_BODY_ID, "cuba:person-girl.v1", "cuba:person-nb.v1"];

pub fn is_valid_body_id(value: &str) -> bool {
    PLAYER_BODY_IDS.contains(&value)
}

pub fn is_valid_date_of_birth(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }
    let year = value[0..4].parse::<u16>().ok();
    let month = value[5..7].parse::<u8>().ok();
    let day = value[8..10].parse::<u8>().ok();
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return false;
    };
    if !(1900..=2100).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=days_in_month).contains(&day)
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodySaveError {
    InvalidBodyId,
    AgeRequired,
    Unauthorized,
    Unavailable,
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BirthdaySaveError {
    InvalidDateOfBirth,
    Unauthorized,
    Unavailable,
    InvalidResponse,
}

impl BirthdaySaveError {
    pub fn from_server_code(code: &str) -> Self {
        match code {
            "invalid_date_of_birth" => Self::InvalidDateOfBirth,
            "not_authenticated" | "unauthorized" => Self::Unauthorized,
            _ => Self::Unavailable,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidDateOfBirth => "invalid_date_of_birth",
            Self::Unauthorized => "unauthorized",
            Self::Unavailable => "unavailable",
            Self::InvalidResponse => "invalid_response",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidDateOfBirth => {
                "That date is not valid. Check the year, month, and day, then try again."
            }
            Self::Unauthorized => "Your sign-in has expired. Please sign in again.",
            Self::Unavailable | Self::InvalidResponse => {
                "We couldn’t save your birthday. Please try again."
            }
        }
    }
}

impl BodySaveError {
    pub fn from_server_code(code: &str) -> Self {
        match code {
            "invalid_body_id" => Self::InvalidBodyId,
            "age_required" => Self::AgeRequired,
            "not_authenticated" | "unauthorized" => Self::Unauthorized,
            _ => Self::Unavailable,
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidBodyId => "Choose one of the available morphs.",
            Self::AgeRequired => "Complete the birthday step before choosing a morph.",
            Self::Unauthorized => "Your sign-in has expired. Please sign in again.",
            Self::Unavailable | Self::InvalidResponse => {
                "We couldn’t save your morph. Please try again."
            }
        }
    }
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyFeedback {
    pub kind: FeedbackKind,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BirthdayFeedback {
    pub kind: FeedbackKind,
    pub code: String,
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
    pub body_id: Option<String>,
    pub body_draft: String,
    pub body_can_save: bool,
    pub body_is_saving: bool,
    pub body_feedback: Option<BodyFeedback>,
    pub date_of_birth: Option<String>,
    pub birthday_is_saving: bool,
    pub birthday_feedback: Option<BirthdayFeedback>,
}

#[derive(Debug)]
struct PendingUsernameSave {
    effect_id: EffectId,
    username: String,
}

#[derive(Debug)]
struct PendingBodySave {
    effect_id: EffectId,
    body_id: String,
}

#[derive(Debug)]
struct PendingBirthdaySave {
    effect_id: EffectId,
    date_of_birth: String,
}

#[derive(Debug)]
pub(crate) struct ProfileState {
    username: Option<String>,
    username_draft: String,
    pending_username_save: Option<PendingUsernameSave>,
    username_feedback: Option<UsernameFeedback>,
    body_id: Option<String>,
    body_draft: String,
    pending_body_save: Option<PendingBodySave>,
    body_feedback: Option<BodyFeedback>,
    date_of_birth: Option<String>,
    pending_birthday_save: Option<PendingBirthdaySave>,
    birthday_feedback: Option<BirthdayFeedback>,
}

impl ProfileState {
    pub(crate) fn new(
        username: Option<String>,
        body_id: Option<String>,
        date_of_birth: Option<String>,
    ) -> Self {
        let username = clean_current_username(username);
        let body_id = clean_body_id(body_id);
        let date_of_birth = clean_date_of_birth(date_of_birth);
        Self {
            username_draft: username.clone().unwrap_or_default(),
            username,
            pending_username_save: None,
            username_feedback: None,
            body_draft: body_id
                .clone()
                .unwrap_or_else(|| DEFAULT_BODY_ID.to_owned()),
            body_id,
            pending_body_save: None,
            body_feedback: None,
            date_of_birth,
            pending_birthday_save: None,
            birthday_feedback: None,
        }
    }

    pub(crate) fn replace(
        &mut self,
        username: Option<String>,
        body_id: Option<String>,
        date_of_birth: Option<String>,
    ) {
        *self = Self::new(username, body_id, date_of_birth);
    }

    pub(crate) fn change_username(&mut self, value: String) {
        self.username_draft = value;
        self.username_feedback = None;
    }

    pub(crate) fn begin_username_edit(&mut self) {
        // Navigation never cancels a save. An idle editor gets a fresh draft.
        if self.pending_username_save.is_none() {
            self.username_draft = self.username.clone().unwrap_or_default();
            self.username_feedback = None;
        }
    }

    pub(crate) fn is_pending(&self, effect_id: EffectId) -> bool {
        self.pending_username_save
            .as_ref()
            .is_some_and(|pending| pending.effect_id == effect_id)
    }

    pub(crate) fn request_username_save(&mut self, effect_id: EffectId) -> Option<String> {
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
        Some(username)
    }

    pub(crate) fn username_saved(&mut self, effect_id: EffectId, username: String) {
        let Some(pending) = self.take_matching_save(effect_id) else {
            return;
        };
        let draft_matches_submission = validate_account_username(&self.username_draft)
            .is_ok_and(|draft| draft == pending.username);
        if validate_account_username(&username).is_err() || username != pending.username {
            self.username_feedback = draft_matches_submission
                .then(|| UsernameFeedback::save_failed(UsernameSaveError::InvalidResponse));
            return;
        }
        self.username = Some(username.clone());
        if draft_matches_submission {
            self.username_draft = username;
        }
        self.username_feedback = draft_matches_submission.then(UsernameFeedback::saved);
    }

    pub(crate) fn username_save_failed(&mut self, effect_id: EffectId, error: UsernameSaveError) {
        if let Some(pending) = self.take_matching_save(effect_id) {
            self.username_feedback = validate_account_username(&self.username_draft)
                .is_ok_and(|draft| draft == pending.username)
                .then(|| UsernameFeedback::save_failed(error));
        }
    }

    pub(crate) fn begin_body_edit(&mut self) {
        if self.pending_body_save.is_none() {
            self.body_draft = self
                .body_id
                .clone()
                .unwrap_or_else(|| DEFAULT_BODY_ID.to_owned());
            self.body_feedback = None;
        }
    }

    pub(crate) fn change_body(&mut self, value: String) {
        self.body_draft = value;
        self.body_feedback = None;
    }

    pub(crate) fn is_body_pending(&self, effect_id: EffectId) -> bool {
        self.pending_body_save
            .as_ref()
            .is_some_and(|pending| pending.effect_id == effect_id)
    }

    pub(crate) fn pending_body_id(&self, effect_id: EffectId) -> Option<&str> {
        self.pending_body_save
            .as_ref()
            .filter(|pending| pending.effect_id == effect_id)
            .map(|pending| pending.body_id.as_str())
    }

    pub(crate) fn request_body_save(&mut self, effect_id: EffectId) -> Option<String> {
        if self.pending_body_save.is_some() {
            return None;
        }
        if !is_valid_body_id(&self.body_draft) {
            self.body_feedback = Some(BodyFeedback {
                kind: FeedbackKind::Error,
                message: BodySaveError::InvalidBodyId.message().to_owned(),
            });
            return None;
        }
        if self.body_id.as_deref() == Some(self.body_draft.as_str()) {
            return None;
        }
        self.body_feedback = None;
        self.pending_body_save = Some(PendingBodySave {
            effect_id,
            body_id: self.body_draft.clone(),
        });
        Some(self.body_draft.clone())
    }

    pub(crate) fn body_saved(&mut self, effect_id: EffectId, body_id: String) {
        let Some(pending) = self.take_matching_body_save(effect_id) else {
            return;
        };
        let draft_matches_submission = self.body_draft == pending.body_id;
        if !is_valid_body_id(&body_id) || body_id != pending.body_id {
            if draft_matches_submission {
                self.body_feedback = Some(BodyFeedback {
                    kind: FeedbackKind::Error,
                    message: BodySaveError::InvalidResponse.message().to_owned(),
                });
            }
            return;
        }
        self.body_id = Some(body_id.clone());
        if draft_matches_submission {
            self.body_draft = body_id;
            self.body_feedback = Some(BodyFeedback {
                kind: FeedbackKind::Success,
                message: "Morph saved.".to_owned(),
            });
        }
    }

    pub(crate) fn body_save_failed(&mut self, effect_id: EffectId, error: BodySaveError) {
        let matches = self
            .take_matching_body_save(effect_id)
            .is_some_and(|pending| self.body_draft == pending.body_id);
        if matches {
            self.body_feedback = Some(BodyFeedback {
                kind: FeedbackKind::Error,
                message: error.message().to_owned(),
            });
        }
    }

    pub(crate) fn request_birthday_save(
        &mut self,
        effect_id: EffectId,
        date_of_birth: String,
    ) -> Option<String> {
        if self.pending_birthday_save.is_some() {
            return None;
        }
        if !is_valid_date_of_birth(&date_of_birth) {
            self.birthday_feedback = Some(BirthdayFeedback {
                kind: FeedbackKind::Error,
                code: BirthdaySaveError::InvalidDateOfBirth.code().to_owned(),
                message: BirthdaySaveError::InvalidDateOfBirth.message().to_owned(),
            });
            return None;
        }
        if self.date_of_birth.as_deref() == Some(date_of_birth.as_str()) {
            return None;
        }
        self.birthday_feedback = None;
        self.pending_birthday_save = Some(PendingBirthdaySave {
            effect_id,
            date_of_birth: date_of_birth.clone(),
        });
        Some(date_of_birth)
    }

    pub(crate) fn is_birthday_pending(&self, effect_id: EffectId) -> bool {
        self.pending_birthday_save
            .as_ref()
            .is_some_and(|pending| pending.effect_id == effect_id)
    }

    pub(crate) fn pending_birthday_date_of_birth(&self, effect_id: EffectId) -> Option<&str> {
        self.pending_birthday_save
            .as_ref()
            .filter(|pending| pending.effect_id == effect_id)
            .map(|pending| pending.date_of_birth.as_str())
    }

    pub(crate) fn birthday_saved(&mut self, effect_id: EffectId, date_of_birth: String) {
        let Some(pending) = self.take_matching_birthday_save(effect_id) else {
            return;
        };
        if !is_valid_date_of_birth(&date_of_birth) || date_of_birth != pending.date_of_birth {
            self.birthday_feedback = Some(BirthdayFeedback {
                kind: FeedbackKind::Error,
                code: BirthdaySaveError::InvalidResponse.code().to_owned(),
                message: BirthdaySaveError::InvalidResponse.message().to_owned(),
            });
            return;
        }
        self.date_of_birth = Some(date_of_birth);
        self.birthday_feedback = Some(BirthdayFeedback {
            kind: FeedbackKind::Success,
            code: "saved".to_owned(),
            message: "Birthday saved.".to_owned(),
        });
    }

    pub(crate) fn birthday_save_failed(&mut self, effect_id: EffectId, error: BirthdaySaveError) {
        if self.take_matching_birthday_save(effect_id).is_some() {
            self.birthday_feedback = Some(BirthdayFeedback {
                kind: FeedbackKind::Error,
                code: error.code().to_owned(),
                message: error.message().to_owned(),
            });
        }
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
            body_id: self.body_id.clone(),
            body_draft: self.body_draft.clone(),
            body_can_save: self.pending_body_save.is_none()
                && is_valid_body_id(&self.body_draft)
                && self.body_id.as_deref() != Some(self.body_draft.as_str()),
            body_is_saving: self.pending_body_save.is_some(),
            body_feedback: self.body_feedback.clone(),
            date_of_birth: self.date_of_birth.clone(),
            birthday_is_saving: self.pending_birthday_save.is_some(),
            birthday_feedback: self.birthday_feedback.clone(),
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

    fn take_matching_body_save(&mut self, effect_id: EffectId) -> Option<PendingBodySave> {
        if self
            .pending_body_save
            .as_ref()
            .is_none_or(|pending| pending.effect_id != effect_id)
        {
            return None;
        }
        self.pending_body_save.take()
    }

    fn take_matching_birthday_save(&mut self, effect_id: EffectId) -> Option<PendingBirthdaySave> {
        if self
            .pending_birthday_save
            .as_ref()
            .is_none_or(|pending| pending.effect_id != effect_id)
        {
            return None;
        }
        self.pending_birthday_save.take()
    }
}

fn clean_current_username(username: Option<String>) -> Option<String> {
    username.and_then(|username| {
        let username = username.trim();
        (!username.is_empty()).then(|| username.to_owned())
    })
}

fn clean_body_id(body_id: Option<String>) -> Option<String> {
    body_id.filter(|body_id| is_valid_body_id(body_id))
}

fn clean_date_of_birth(date_of_birth: Option<String>) -> Option<String> {
    date_of_birth.filter(|date_of_birth| is_valid_date_of_birth(date_of_birth))
}
