use serde::{Deserialize, Serialize};

use crate::EffectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyPendingAction {
    Load,
    Block,
    Unblock,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyFeedback {
    pub kind: SafetyFeedbackKind,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyFeedbackKind {
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetySnapshot {
    pub blocked_user_ids: Vec<String>,
    pub is_loading: bool,
    pub pending_action: Option<SafetyPendingAction>,
    pub pending_user_id: Option<String>,
    pub feedback: Option<SafetyFeedback>,
}

#[derive(Debug)]
enum PendingSafetyAction {
    Load,
    Block { user_id: String },
    Unblock { user_id: String, index: usize },
}

#[derive(Debug, Default)]
pub(crate) struct SafetyState {
    blocked_user_ids: Vec<String>,
    pending: Option<(EffectId, PendingSafetyAction)>,
    feedback: Option<SafetyFeedback>,
}

impl SafetyState {
    pub(crate) fn replace(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn request_load(&mut self, effect_id: EffectId) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.feedback = None;
        self.pending = Some((effect_id, PendingSafetyAction::Load));
        true
    }

    pub(crate) fn request_block(&mut self, effect_id: EffectId, raw_user_id: &str) -> bool {
        let Some(user_id) = normalize_user_id(raw_user_id) else {
            self.set_error("invalid_block_target", "That player could not be blocked.");
            return false;
        };
        if self.pending.is_some() || self.blocked_user_ids.iter().any(|id| id == &user_id) {
            return false;
        }
        self.feedback = None;
        self.blocked_user_ids.push(user_id.clone());
        self.pending = Some((effect_id, PendingSafetyAction::Block { user_id }));
        true
    }

    pub(crate) fn request_unblock(&mut self, effect_id: EffectId, raw_user_id: &str) -> bool {
        let Some(user_id) = normalize_user_id(raw_user_id) else {
            self.set_error(
                "invalid_block_target",
                "That player could not be unblocked.",
            );
            return false;
        };
        if self.pending.is_some() {
            return false;
        }
        let Some(index) = self
            .blocked_user_ids
            .iter()
            .position(|blocked_id| blocked_id == &user_id)
        else {
            return false;
        };
        self.feedback = None;
        self.blocked_user_ids.remove(index);
        self.pending = Some((effect_id, PendingSafetyAction::Unblock { user_id, index }));
        true
    }

    pub(crate) fn is_pending(&self, effect_id: EffectId) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(pending_id, _)| *pending_id == effect_id)
    }

    pub(crate) fn pending_action(&self, effect_id: EffectId) -> Option<SafetyPendingAction> {
        let (pending_id, action) = self.pending.as_ref()?;
        if *pending_id != effect_id {
            return None;
        }
        Some(match action {
            PendingSafetyAction::Load => SafetyPendingAction::Load,
            PendingSafetyAction::Block { .. } => SafetyPendingAction::Block,
            PendingSafetyAction::Unblock { .. } => SafetyPendingAction::Unblock,
        })
    }

    pub(crate) fn invalid_request(&mut self, code: &str, message: &str) {
        self.set_error(code, message);
    }

    pub(crate) fn loaded(&mut self, effect_id: EffectId, ids: Vec<String>) {
        let Some((pending_id, PendingSafetyAction::Load)) = self.take_matching(effect_id) else {
            return;
        };
        debug_assert_eq!(pending_id, effect_id);
        self.blocked_user_ids = normalize_user_ids(ids);
        self.feedback = None;
    }

    pub(crate) fn action_succeeded(&mut self, effect_id: EffectId) {
        if self.take_matching(effect_id).is_some() {
            self.feedback = None;
        }
    }

    pub(crate) fn failed(&mut self, effect_id: EffectId, code: &str, message: &str) {
        let Some((pending_id, pending)) = self.take_matching(effect_id) else {
            return;
        };
        debug_assert_eq!(pending_id, effect_id);
        match pending {
            PendingSafetyAction::Load => {}
            PendingSafetyAction::Block { user_id } => {
                self.blocked_user_ids.retain(|id| id != &user_id);
            }
            PendingSafetyAction::Unblock { user_id, index } => {
                let insertion_index = index.min(self.blocked_user_ids.len());
                if !self.blocked_user_ids.iter().any(|id| id == &user_id) {
                    self.blocked_user_ids.insert(insertion_index, user_id);
                }
            }
        }
        self.set_error(code, message);
    }

    pub(crate) fn snapshot(&self) -> SafetySnapshot {
        let (pending_action, pending_user_id) =
            match self.pending.as_ref().map(|(_, action)| action) {
                Some(PendingSafetyAction::Load) => (Some(SafetyPendingAction::Load), None),
                Some(PendingSafetyAction::Block { user_id }) => {
                    (Some(SafetyPendingAction::Block), Some(user_id.clone()))
                }
                Some(PendingSafetyAction::Unblock { user_id, .. }) => {
                    (Some(SafetyPendingAction::Unblock), Some(user_id.clone()))
                }
                None => (None, None),
            };
        SafetySnapshot {
            blocked_user_ids: self.blocked_user_ids.clone(),
            is_loading: self.pending.is_some(),
            pending_action,
            pending_user_id,
            feedback: self.feedback.clone(),
        }
    }

    fn take_matching(&mut self, effect_id: EffectId) -> Option<(EffectId, PendingSafetyAction)> {
        let (pending_id, _) = self.pending.as_ref()?;
        if *pending_id != effect_id {
            return None;
        }
        self.pending.take()
    }

    fn set_error(&mut self, code: &str, message: &str) {
        self.feedback = Some(SafetyFeedback {
            kind: SafetyFeedbackKind::Error,
            code: code.to_owned(),
            message: message.to_owned(),
        });
    }
}

pub(crate) fn blocked_users_response(
    status: u16,
    body: &str,
) -> Result<Vec<String>, (&'static str, &'static str)> {
    if let Some(error) = response_error(
        status,
        body,
        "We couldn’t load your blocked users. Please try again.",
    ) {
        return Err(error);
    }
    let response: BlockedUsersResponse = serde_json::from_str(body).map_err(|_| {
        (
            "invalid_response",
            "We couldn’t load your blocked users. Please try again.",
        )
    })?;
    Ok(normalize_user_ids(response.user_ids))
}

pub(crate) fn action_response(
    status: u16,
    body: &str,
    fallback_message: &'static str,
) -> Result<(), (&'static str, &'static str)> {
    if let Some(error) = response_error(status, body, fallback_message) {
        return Err(error);
    }
    let response: SafetyActionResponse =
        serde_json::from_str(body).map_err(|_| ("invalid_response", fallback_message))?;
    if response.ok {
        Ok(())
    } else {
        Err(("invalid_response", fallback_message))
    }
}

#[derive(Deserialize)]
struct BlockedUsersResponse {
    user_ids: Vec<String>,
}

#[derive(Deserialize)]
struct SafetyActionResponse {
    ok: bool,
}

fn response_error(
    status: u16,
    body: &str,
    fallback_message: &'static str,
) -> Option<(&'static str, &'static str)> {
    if (200..300).contains(&status) {
        return None;
    }
    if status == 401 {
        return Some((
            "not_authenticated",
            "Your session has expired. Please sign in again.",
        ));
    }
    if status == 403 {
        return Some((
            "age_required",
            "Complete your birthday before using player safety.",
        ));
    }
    let code = serde_json::from_str::<ServerError>(body)
        .ok()
        .and_then(|error| match error.error.as_str() {
            "invalid_block_target" => Some("invalid_block_target"),
            _ => None,
        })
        .unwrap_or("unavailable");
    Some((code, fallback_message))
}

#[derive(Deserialize)]
struct ServerError {
    error: String,
}

fn normalize_user_ids(ids: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(id) = normalize_user_id(&id) else {
            continue;
        };
        if !normalized.iter().any(|existing| existing == &id) {
            normalized.push(id);
        }
    }
    normalized
}

fn normalize_user_id(value: &str) -> Option<String> {
    let id = value.trim();
    if id.is_empty() || id.len() > 128 {
        None
    } else {
        Some(id.to_owned())
    }
}

pub(crate) fn normalize_requested_user_id(value: &str) -> Option<String> {
    normalize_user_id(value)
}
