mod profile;
mod username;

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

pub use profile::{
    FeedbackKind, ProfileSnapshot, UsernameFeedback, UsernameFeedbackCode, UsernameSaveError,
};
pub use username::{
    USERNAME_MAX_CHARACTERS, USERNAME_MIN_CHARACTERS, UsernameValidationError,
    validate_account_username,
};

use profile::ProfileState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppAction {
    ReplaceProfile {
        username: Option<String>,
    },
    UsernameChanged {
        value: String,
    },
    SaveUsername,
    UsernameSaved {
        effect_id: EffectId,
        username: String,
    },
    UsernameSaveFailed {
        effect_id: EffectId,
        error: UsernameSaveError,
    },
    ClearUsernameFeedback,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEffect {
    SaveUsername {
        effect_id: EffectId,
        username: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub profile: ProfileSnapshot,
}

/// Platform-neutral application state for native and web presentation layers.
///
/// The host renders snapshots, performs queued effects using its native HTTP
/// and credential stack, and dispatches the typed result back into this model.
pub struct AppModel {
    profile: ProfileState,
    effects: VecDeque<AppEffect>,
    next_effect_id: u32,
}

impl AppModel {
    pub fn new(username: Option<String>) -> Self {
        Self {
            profile: ProfileState::new(username),
            effects: VecDeque::new(),
            next_effect_id: 1,
        }
    }

    pub fn dispatch(&mut self, action: AppAction) {
        match action {
            AppAction::ReplaceProfile { username } => {
                self.profile.replace(username);
                self.effects.clear();
            }
            AppAction::UsernameChanged { value } => self.profile.change_username(value),
            AppAction::SaveUsername => {
                let effect_id = self.allocate_effect_id();
                if let Some(effect) = self.profile.request_username_save(effect_id) {
                    self.effects.push_back(effect);
                }
            }
            AppAction::UsernameSaved {
                effect_id,
                username,
            } => self.profile.username_saved(effect_id, username),
            AppAction::UsernameSaveFailed { effect_id, error } => {
                self.profile.username_save_failed(effect_id, error);
            }
            AppAction::ClearUsernameFeedback => self.profile.clear_username_feedback(),
        }
    }

    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            profile: self.profile.snapshot(),
        }
    }

    pub fn take_effects(&mut self) -> Vec<AppEffect> {
        self.effects.drain(..).collect()
    }

    fn allocate_effect_id(&mut self) -> EffectId {
        let effect_id = EffectId(self.next_effect_id);
        self.next_effect_id = self.next_effect_id.wrapping_add(1);
        if self.next_effect_id == 0 {
            self.next_effect_id = 1;
        }
        effect_id
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppAction, AppEffect, AppModel, FeedbackKind, UsernameFeedbackCode, UsernameSaveError,
        UsernameValidationError,
    };

    fn begin_save(app: &mut AppModel, draft: &str) -> (super::EffectId, String) {
        app.dispatch(AppAction::UsernameChanged {
            value: draft.to_owned(),
        });
        app.dispatch(AppAction::SaveUsername);
        let effects = app.take_effects();
        let Some(AppEffect::SaveUsername {
            effect_id,
            username,
        }) = effects.into_iter().next()
        else {
            panic!("expected a username save effect");
        };
        (effect_id, username)
    }

    #[test]
    fn invalid_username_stays_in_rust_and_exposes_renderable_feedback() {
        let mut app = AppModel::new(Some("Ada".to_owned()));
        app.dispatch(AppAction::UsernameChanged {
            value: "a".to_owned(),
        });
        app.dispatch(AppAction::SaveUsername);

        assert!(app.take_effects().is_empty());
        let profile = app.snapshot().profile;
        assert_eq!(
            profile.username_validation_error,
            Some(UsernameValidationError::TooShort)
        );
        assert!(!profile.username_can_save);
        let feedback = profile.username_feedback.expect("validation feedback");
        assert_eq!(feedback.kind, FeedbackKind::Error);
        assert_eq!(
            feedback.code,
            UsernameFeedbackCode::Validation {
                error: UsernameValidationError::TooShort
            }
        );
        assert_eq!(feedback.message, "Use 2–24 letters, numbers, _ or -.");
    }

    #[test]
    fn queues_normalized_save_and_updates_snapshot_after_success() {
        let mut app = AppModel::new(Some("Ada".to_owned()));
        let (effect_id, username) = begin_save(&mut app, "  Grace_7  ");
        assert_eq!(username, "Grace_7");
        assert!(app.snapshot().profile.username_is_saving);

        app.dispatch(AppAction::UsernameSaved {
            effect_id,
            username: "Grace_7".to_owned(),
        });

        let profile = app.snapshot().profile;
        assert_eq!(profile.username.as_deref(), Some("Grace_7"));
        assert_eq!(profile.username_draft, "Grace_7");
        assert!(!profile.username_is_dirty);
        assert!(!profile.username_is_saving);
        assert_eq!(
            profile.username_feedback.expect("saved feedback").code,
            UsernameFeedbackCode::Saved
        );
    }

    #[test]
    fn maps_save_failures_without_losing_the_draft() {
        let mut app = AppModel::new(Some("Ada".to_owned()));
        let (effect_id, _) = begin_save(&mut app, "Grace");
        app.dispatch(AppAction::UsernameSaveFailed {
            effect_id,
            error: UsernameSaveError::Taken,
        });

        let profile = app.snapshot().profile;
        assert_eq!(profile.username.as_deref(), Some("Ada"));
        assert_eq!(profile.username_draft, "Grace");
        assert!(profile.username_can_save);
        assert_eq!(
            profile.username_feedback.expect("failure feedback").message,
            "That username is already in use. Try another."
        );
    }

    #[test]
    fn ignores_stale_results_after_the_profile_is_replaced() {
        let mut app = AppModel::new(Some("Ada".to_owned()));
        let (effect_id, _) = begin_save(&mut app, "Grace");
        app.dispatch(AppAction::ReplaceProfile {
            username: Some("Lin".to_owned()),
        });
        app.dispatch(AppAction::UsernameSaved {
            effect_id,
            username: "Grace".to_owned(),
        });

        let profile = app.snapshot().profile;
        assert_eq!(profile.username.as_deref(), Some("Lin"));
        assert_eq!(profile.username_draft, "Lin");
        assert!(profile.username_feedback.is_none());
    }

    #[test]
    fn preserves_new_edits_when_an_older_save_completes() {
        let mut app = AppModel::new(Some("Ada".to_owned()));
        let (effect_id, _) = begin_save(&mut app, "Grace");
        app.dispatch(AppAction::UsernameChanged {
            value: "Lin".to_owned(),
        });
        app.dispatch(AppAction::UsernameSaved {
            effect_id,
            username: "Grace".to_owned(),
        });

        let profile = app.snapshot().profile;
        assert_eq!(profile.username.as_deref(), Some("Grace"));
        assert_eq!(profile.username_draft, "Lin");
        assert!(profile.username_is_dirty);
        assert!(profile.username_can_save);
    }

    #[test]
    fn preserves_an_existing_name_outside_the_stricter_profile_editing_rules() {
        let app = AppModel::new(Some("Ada Lovelace".to_owned()));

        let profile = app.snapshot().profile;
        assert_eq!(profile.username.as_deref(), Some("Ada Lovelace"));
        assert!(!profile.username_is_dirty);
        assert!(!profile.username_can_save);
        assert_eq!(
            profile.username_validation_error,
            Some(UsernameValidationError::InvalidCharacters)
        );
    }
}
