use crate::{AppEffect, BirthdaySaveError, BodySaveError, EffectId, UsernameSaveError};
use serde::Deserialize;

pub(crate) fn username_request(
    effect_id: EffectId,
    account_id: String,
    username: String,
) -> AppEffect {
    AppEffect::HttpRequest {
        effect_id,
        account_id: Some(account_id),
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
        account_id: Some(account_id),
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

pub(crate) fn catalog_request(
    effect_id: EffectId,
    account_id: Option<String>,
    page_size: u16,
) -> AppEffect {
    AppEffect::HttpRequest {
        effect_id,
        account_id,
        method: "GET".into(),
        path: format!("cubes?page=1&page_size={page_size}"),
        body: String::new(),
    }
}

pub(crate) fn birthday_request(
    effect_id: EffectId,
    account_id: String,
    date_of_birth: String,
) -> AppEffect {
    AppEffect::HttpRequest {
        effect_id,
        account_id: Some(account_id),
        method: "POST".into(),
        path: "auth/birthday".into(),
        body: serde_json::json!({ "dob": date_of_birth }).to_string(),
    }
}

pub(crate) fn birthday_response(
    status: u16,
    body: &str,
    account_id: Option<&str>,
    expected_date_of_birth: &str,
) -> Result<String, BirthdaySaveError> {
    if status == 401 {
        return Err(BirthdaySaveError::Unauthorized);
    }
    if !(200..300).contains(&status) {
        #[derive(Deserialize)]
        struct ErrorResponse {
            error: String,
        }
        return Err(serde_json::from_str::<ErrorResponse>(body)
            .map(|response| BirthdaySaveError::from_server_code(&response.error))
            .unwrap_or(BirthdaySaveError::Unavailable));
    }
    #[derive(Deserialize)]
    struct Response {
        user: User,
    }
    #[derive(Deserialize)]
    struct User {
        id: String,
        dob: String,
    }
    let response: Response =
        serde_json::from_str(body).map_err(|_| BirthdaySaveError::InvalidResponse)?;
    if Some(response.user.id.as_str()) != account_id
        || response.user.dob != expected_date_of_birth
        || !crate::is_valid_date_of_birth(&response.user.dob)
    {
        return Err(BirthdaySaveError::InvalidResponse);
    }
    Ok(response.user.dob)
}
