use serde::{Deserialize, Serialize};

pub const USERNAME_MIN_CHARACTERS: usize = 2;
pub const USERNAME_MAX_CHARACTERS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsernameValidationError {
    TooShort,
    TooLong,
    InvalidCharacters,
}

impl UsernameValidationError {
    pub const fn message(self) -> &'static str {
        "Use 2–24 letters, numbers, _ or -."
    }
}

/// Normalizes and validates an account-profile username.
///
/// Account profile screens currently accept ASCII letters, numbers, `_`, and
/// `-`. In-world name editing is a separate product flow and permits spaces.
pub fn validate_account_username(value: &str) -> Result<String, UsernameValidationError> {
    let normalized = value.trim();
    let character_count = normalized.chars().count();
    if character_count < USERNAME_MIN_CHARACTERS {
        return Err(UsernameValidationError::TooShort);
    }
    if character_count > USERNAME_MAX_CHARACTERS {
        return Err(UsernameValidationError::TooLong);
    }
    if !normalized
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(UsernameValidationError::InvalidCharacters);
    }
    Ok(normalized.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{UsernameValidationError, validate_account_username};

    #[test]
    fn trims_valid_account_usernames() {
        assert_eq!(
            validate_account_username("  Ada_24-7\n"),
            Ok("Ada_24-7".to_owned())
        );
    }

    #[test]
    fn rejects_out_of_range_and_non_account_characters() {
        assert_eq!(
            validate_account_username("a"),
            Err(UsernameValidationError::TooShort)
        );
        assert_eq!(
            validate_account_username("abcdefghijklmnopqrstuvwxyz"),
            Err(UsernameValidationError::TooLong)
        );
        assert_eq!(
            validate_account_username("Ada Lovelace"),
            Err(UsernameValidationError::InvalidCharacters)
        );
        assert_eq!(
            validate_account_username("åda"),
            Err(UsernameValidationError::InvalidCharacters)
        );
    }
}
