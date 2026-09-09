use crate::{AppEffect, BodySaveError, EffectId, UsernameSaveError};
use serde::Deserialize;

pub(crate) fn username_request(
    effect_id: EffectId,
    account_id: String,
    username: String,
) -> AppEffect {
    AppEffect::HttpRequest {
        effect_id,
        account_id,
        method: "POST".into(),
        path: "auth/username".into(),
        body: serde_json::json!({ "username": username }).to_string(),
    }
}

pub(crate) fn username_response(
    status: u16,
    body: &str,
    account_id: Option<&str>,
) -> Result<String, UsernameSaveError> {
    if status == 401 {
        return Err(UsernameSaveError::Unauthorized);
    }
    if !(200..300).contains(&status) {
        #[derive(Deserialize)]
        struct ErrorResponse {
            error: String,
        }
        return Err(serde_json::from_str::<ErrorResponse>(body)
            .map(|response| UsernameSaveError::from_server_code(&response.error))
            .unwrap_or(UsernameSaveError::Unavailable));
    }
    #[derive(Deserialize)]
    struct Response {
        user: User,
    }
    #[derive(Deserialize)]
    struct User {
        id: String,
        username: String,
    }
    let response: Response =
        serde_json::from_str(body).map_err(|_| UsernameSaveError::InvalidResponse)?;
    if Some(response.user.id.as_str()) != account_id {
        return Err(UsernameSaveError::InvalidResponse);
    }
    Ok(response.user.username)
}

pub(crate) fn body_request(effect_id: EffectId, account_id: String, body_id: String) -> AppEffect {
    AppEffect::HttpRequest {
        effect_id,
        account_id,
        method: "POST".into(),
        path: "auth/avatar".into(),
        body: serde_json::json!({ "body_id": body_id }).to_string(),
    }
}

pub(crate) fn body_response(
    status: u16,
    body: &str,
    account_id: Option<&str>,
    expected_body_id: &str,
) -> Result<String, BodySaveError> {
    if status == 401 {
        return Err(BodySaveError::Unauthorized);
    }
    if !(200..300).contains(&status) {
        #[derive(Deserialize)]
        struct ErrorResponse {
            error: String,
        }
        return Err(serde_json::from_str::<ErrorResponse>(body)
            .map(|response| BodySaveError::from_server_code(&response.error))
            .unwrap_or(BodySaveError::Unavailable));
    }
    #[derive(Deserialize)]
    struct Response {
        user: User,
    }
    #[derive(Deserialize)]
    struct User {
        id: String,
        body_id: String,
    }
    let response: Response =
        serde_json::from_str(body).map_err(|_| BodySaveError::InvalidResponse)?;
    if Some(response.user.id.as_str()) != account_id
        || response.user.body_id != expected_body_id
        || !crate::is_valid_body_id(&response.user.body_id)
    {
        return Err(BodySaveError::InvalidResponse);
    }
    Ok(response.user.body_id)
}
