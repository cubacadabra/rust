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
        date_of_birth: None,
    });
    app.dispatch(AppAction::UsernameChanged {
        value: "Grace".into(),
    });
    app.dispatch(AppAction::SaveUsername {});
    app.dispatch(AppAction::ReplaceSession {
        account_id: None,
        username: None,
        body_id: None,
        date_of_birth: None,
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
        date_of_birth: None,
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

#[test]
fn birthday_save_uses_shared_validation_and_response_contract() {
    let mut app = AppModel::default();
    app.dispatch(AppAction::ReplaceSession {
        account_id: Some("a".into()),
        username: Some("Ada".into()),
        body_id: None,
        date_of_birth: None,
    });
    app.dispatch(AppAction::SaveBirthday {
        date_of_birth: "2001-02-29".into(),
    });
    assert_eq!(
        app.snapshot()
            .profile
            .birthday_feedback
            .as_ref()
            .unwrap()
            .code,
        "invalid_date_of_birth"
    );
    assert!(app.poll_effect().is_none());

    app.dispatch(AppAction::SaveBirthday {
        date_of_birth: "2000-02-29".into(),
    });
    let effect = app.poll_effect().expect("birthday request");
    let effect_id = match effect {
        AppEffect::HttpRequest {
            effect_id,
            path,
            body,
            ..
        } => {
            assert_eq!(path, "auth/birthday");
            assert_eq!(body, r#"{"dob":"2000-02-29"}"#);
            effect_id
        }
    };
    app.dispatch(AppAction::HttpCompleted {
        effect_id,
        status: 200,
        body: r#"{"user":{"id":"a","dob":"2000-02-29"},"age":25}"#.into(),
    });
    assert_eq!(
        app.snapshot().profile.date_of_birth.as_deref(),
        Some("2000-02-29")
    );
    assert_eq!(
        app.snapshot()
            .profile
            .birthday_feedback
            .as_ref()
            .unwrap()
            .code,
        "saved"
    );
}

#[test]
fn catalog_load_uses_shared_request_and_response_contract() {
    let mut app = AppModel::default();
    app.dispatch(AppAction::ReplaceSession {
        account_id: Some("a".into()),
        username: Some("Ada".into()),
        body_id: None,
        date_of_birth: None,
    });
    app.dispatch(AppAction::LoadCatalog { page_size: 20 });
    assert!(app.snapshot().catalog.is_loading);
    let effect = app.poll_effect().expect("catalog request");
    let effect_id = match effect {
        AppEffect::HttpRequest {
            effect_id,
            method,
            path,
            body,
            ..
        } => {
            assert_eq!(method, "GET");
            assert_eq!(path, "cubes?page=1&page_size=20");
            assert!(body.is_empty());
            effect_id
        }
    };
    app.dispatch(AppAction::HttpCompleted {
        effect_id,
        status: 200,
        body: r#"{"cubes":[
            {"id":1,"cubeId":"first-game","version":"0.3.0","displayName":"First Game","fileCount":2,"packagePath":"/cubes/first-game/0.3.0/"},
            {"id":2,"cubeId":"first-game","version":"0.2.0","displayName":"Old First Game","fileCount":2,"packagePath":"/cubes/first-game/0.2.0/"},
            {"id":3,"cubeId":"Bad_Name","version":"1","displayName":"Bad","fileCount":1,"packagePath":"/cubes/bad/1/"}
        ]}"#.into(),
    });
    let catalog = app.snapshot().catalog;
    assert!(!catalog.is_loading);
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].cube_id, "first-game");
    assert_eq!(catalog.entries[0].package_path, "/cubes/first-game/0.3.0/");
    assert!(catalog.feedback.is_none());
}

#[test]
fn catalog_load_is_available_to_guest_hosts() {
    let mut app = AppModel::default();
    app.dispatch(AppAction::LoadCatalog { page_size: 20 });
    let effect = app.poll_effect().expect("guest catalog request");
    assert!(matches!(
        effect,
        AppEffect::HttpRequest {
            account_id: None,
            method,
            path,
            ..
        } if method == "GET" && path == "cubes?page=1&page_size=20"
    ));
    assert!(app.snapshot().catalog.is_loading);
}
