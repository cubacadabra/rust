use crate::{AppAction, AppEffect, AppModel, EffectId};
use serde_json::Value;

fn assert_subset(actual: &Value, expected: &Value) {
    if let Some(fields) = expected.as_object() {
        for (key, value) in fields {
            assert_subset(&actual[key], value);
        }
    } else {
        assert_eq!(actual, expected);
    }
}

#[test]
fn shared_host_contract() {
    let scenarios: Value =
        serde_json::from_str(include_str!("../tests/username-contract.json")).unwrap();
    for scenario in scenarios.as_array().unwrap() {
        let mut app = AppModel::default();
        for step in scenario["steps"].as_array().unwrap() {
            app.dispatch_json(&step["action"].to_string()).unwrap();
            assert_subset(
                &serde_json::to_value(app.snapshot()).unwrap(),
                &step["expected"],
            );
            assert_eq!(
                serde_json::to_value(app.take_effects()).unwrap(),
                step["effects"],
                "{}",
                scenario["name"]
            );
        }
    }
}

#[test]
fn replacement_drops_undelivered_effects() {
    let mut app = AppModel::default();
    app.dispatch(AppAction::ReplaceSession {
        account_id: Some("a".into()),
        username: None,
        body_id: None,
    });
    app.dispatch(AppAction::UsernameChanged {
        value: "Grace".into(),
    });
    app.dispatch(AppAction::SaveUsername {});
    app.dispatch(AppAction::ReplaceSession {
        account_id: None,
        username: None,
        body_id: None,
    });
    assert!(app.take_effects().is_empty());
}

#[test]
fn body_save_uses_shared_validation_and_preserves_newer_draft() {
    let mut app = AppModel::default();
    app.dispatch(AppAction::ReplaceSession {
        account_id: Some("a".into()),
        username: Some("Ada".into()),
        body_id: Some("cuba:person.v1".into()),
    });
    app.dispatch(AppAction::BeginBodyEdit {});
    assert_eq!(
        app.snapshot().profile.body_id.as_deref(),
        Some("cuba:person.v1")
    );
    app.dispatch(AppAction::BodyChanged {
        body_id: "cuba:person-girl.v1".into(),
    });
    app.dispatch(AppAction::SaveBody {});
    let effect = app.poll_effect().expect("body request");
    assert!(matches!(effect, AppEffect::HttpRequest { path, .. } if path == "auth/avatar"));
    app.dispatch(AppAction::BodyChanged {
        body_id: "cuba:person-nb.v1".into(),
    });
    app.dispatch(AppAction::HttpCompleted {
        effect_id: EffectId(1),
        status: 200,
        body: r#"{"user":{"id":"a","body_id":"cuba:person-girl.v1"}}"#.into(),
    });
    let profile = app.snapshot().profile;
    assert_eq!(profile.body_id.as_deref(), Some("cuba:person-girl.v1"));
    assert_eq!(profile.body_draft, "cuba:person-nb.v1");
    assert!(profile.body_can_save);
    assert!(profile.body_feedback.is_none());
}
