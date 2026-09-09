use crate::{AppAction, AppModel};
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
    });
    app.dispatch(AppAction::UsernameChanged {
        value: "Grace".into(),
    });
    app.dispatch(AppAction::SaveUsername {});
    app.dispatch(AppAction::ReplaceSession {
        account_id: None,
        username: None,
    });
    assert!(app.take_effects().is_empty());
}
