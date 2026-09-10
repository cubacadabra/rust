use serde::{Deserialize, Deserializer, Serialize, de};
use std::{error::Error, fmt, str::FromStr};

/// Maximum size retained from the version-1 appearance wire contract.
pub const MAX_ASSET_ID_BYTES: usize = 96;

/// A bounded asset identifier safe to carry in packages and appearance data.
///
/// `parse` intentionally accepts the version-1 identifier envelope. New
/// published morph assets should additionally pass `validate_published`, which
/// requires a namespace and a positive `.vN` suffix.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MorphAssetId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetIdError {
    Empty,
    TooLong,
    NonAscii,
    InvalidCharacter,
    MissingNamespace,
    InvalidNamespace,
    MissingVersion,
    InvalidVersion,
    InvalidPath,
}

impl MorphAssetId {
    pub fn parse(value: impl Into<String>) -> Result<Self, AssetIdError> {
        let value = value.into();
        validate_envelope(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn validate_published(&self) -> Result<(), AssetIdError> {
        let Some((namespace, path_and_version)) = self.0.split_once(':') else {
            return Err(AssetIdError::MissingNamespace);
        };
        if namespace.is_empty()
            || !namespace
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(AssetIdError::InvalidNamespace);
        }
        let Some((path, version)) = path_and_version.rsplit_once(".v") else {
            return Err(AssetIdError::MissingVersion);
        };
        if path.is_empty()
            || path.starts_with('/')
            || path.ends_with('/')
            || path.split('/').any(|segment| {
                segment.is_empty()
                    || !segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'-' | b'_')
                    })
            })
        {
            return Err(AssetIdError::InvalidPath);
        }
        if version.is_empty()
            || version.starts_with('0')
            || !version.bytes().all(|byte| byte.is_ascii_digit())
            || version.parse::<u32>().is_err()
        {
            return Err(AssetIdError::InvalidVersion);
        }
        Ok(())
    }
}

fn validate_envelope(value: &str) -> Result<(), AssetIdError> {
    if value.is_empty() {
        return Err(AssetIdError::Empty);
    }
    if value.len() > MAX_ASSET_ID_BYTES {
        return Err(AssetIdError::TooLong);
    }
    if !value.is_ascii() {
        return Err(AssetIdError::NonAscii);
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'-' | b'_' | b'/')
    }) {
        return Err(AssetIdError::InvalidCharacter);
    }
    Ok(())
}

impl fmt::Display for MorphAssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for AssetIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "asset ID is empty",
            Self::TooLong => "asset ID exceeds 96 bytes",
            Self::NonAscii => "asset ID must be ASCII",
            Self::InvalidCharacter => "asset ID contains an invalid character",
            Self::MissingNamespace => "published asset ID needs a namespace",
            Self::InvalidNamespace => "published asset ID has an invalid namespace",
            Self::MissingVersion => "published asset ID needs a .vN suffix",
            Self::InvalidVersion => "published asset ID has an invalid version",
            Self::InvalidPath => "published asset ID has an invalid path",
        })
    }
}

impl Error for AssetIdError {}

impl FromStr for MorphAssetId {
    type Err = AssetIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<String> for MorphAssetId {
    type Error = AssetIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl<'de> Deserialize<'de> for MorphAssetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_one_envelope_remains_compatible() {
        for value in [
            "cuba:person.v1",
            "cuba:star-cap.v1",
            "cuba:base/person.v1",
            "draft_asset",
        ] {
            assert_eq!(MorphAssetId::parse(value).unwrap().as_str(), value);
        }
    }

    #[test]
    fn envelope_rejects_unsafe_values() {
        assert_eq!(MorphAssetId::parse(""), Err(AssetIdError::Empty));
        assert_eq!(
            MorphAssetId::parse("cuba:hat with spaces.v1"),
            Err(AssetIdError::InvalidCharacter)
        );
        assert_eq!(
            MorphAssetId::parse("cuba:hat#fragment.v1"),
            Err(AssetIdError::InvalidCharacter)
        );
    }

    #[test]
    fn published_ids_are_namespaced_and_versioned() {
        let id = MorphAssetId::parse("cuba:hair/side-ponytail.v2").unwrap();
        assert_eq!(id.validate_published(), Ok(()));
        assert_eq!(
            MorphAssetId::parse("draft_asset")
                .unwrap()
                .validate_published(),
            Err(AssetIdError::MissingNamespace)
        );
        assert_eq!(
            MorphAssetId::parse("cuba:hair/ponytail.v0")
                .unwrap()
                .validate_published(),
            Err(AssetIdError::InvalidVersion)
        );
    }
}
