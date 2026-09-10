use crate::{MorphAssetId, MorphDiagnostic, MorphLoadout, MorphParameterValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Owned form of the version-1 appearance wire object. This belongs here so
/// hosts and the engine can migrate without duplicating the compatibility map.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyAppearance {
    #[serde(default)]
    pub version: Option<u16>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub face: Option<String>,
    #[serde(default)]
    pub outfit: Option<String>,
    #[serde(default)]
    pub equipment: BTreeMap<String, String>,
    #[serde(default)]
    pub colors: BTreeMap<String, String>,
    #[serde(default)]
    pub revision: u32,
}

pub fn migrate_v1_appearance(
    legacy: &LegacyAppearance,
) -> Result<MorphLoadout, Vec<MorphDiagnostic>> {
    if legacy.version.unwrap_or(1) != 1 {
        return Err(vec![MorphDiagnostic::error(
            "MORPH_LEGACY_UNSUPPORTED_VERSION",
            "version",
            "only version-1 appearances can be migrated",
        )]);
    }
    let body = legacy.body.as_deref().unwrap_or("cuba:person.v1");
    let (base, hair) = match body {
        "cuba:person.v1" => ("cuba:base/person.v1", Some("cuba:hair/swept.v1")),
        "cuba:person-girl.v1" => ("cuba:base/person.v1", Some("cuba:hair/side-ponytail.v1")),
        "cuba:person-nb.v1" => ("cuba:base/person.v1", Some("cuba:hair/shag.v1")),
        "cuba:cat.v1" => ("cuba:base/cat.v1", None),
        "cuba:wolf.v1" => ("cuba:base/wolf.v1", None),
        "cuba:dragon.v1" => ("cuba:base/dragon.v1", None),
        _ => {
            return Err(vec![MorphDiagnostic::error(
                "MORPH_LEGACY_UNKNOWN_BODY",
                "body",
                format!("version-1 body {body:?} has no morph compatibility mapping"),
            )]);
        }
    };
    let mut parts = Vec::new();
    if let Some(hair) = hair {
        parts.push(parse_known_id(hair));
    }
    if let Some(outfit) = legacy.outfit.as_deref() {
        match MorphAssetId::parse(outfit) {
            Ok(outfit) => parts.push(outfit),
            Err(error) => {
                return Err(vec![MorphDiagnostic::error(
                    "MORPH_LEGACY_INVALID_ASSET_ID",
                    "outfit",
                    error.to_string(),
                )]);
            }
        }
    }
    for (slot, item) in &legacy.equipment {
        match MorphAssetId::parse(item) {
            Ok(item) => parts.push(item),
            Err(error) => {
                return Err(vec![MorphDiagnostic::error(
                    "MORPH_LEGACY_INVALID_ASSET_ID",
                    &format!("equipment.{slot}"),
                    error.to_string(),
                )]);
            }
        }
    }
    let face = legacy.face.as_deref().unwrap_or("happy").replace('_', "-");
    if face.is_empty()
        || !face
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(vec![MorphDiagnostic::error(
            "MORPH_LEGACY_INVALID_FACE",
            "face",
            "version-1 face name cannot form a stable morph asset ID",
        )]);
    }
    let parameters = legacy
        .colors
        .iter()
        .map(|(key, value)| (key.clone(), MorphParameterValue::Text(value.clone())))
        .collect();
    let mut loadout = MorphLoadout {
        version: 2,
        base: parse_known_id(base),
        parts,
        face: Some(parse_known_id(&format!("cuba:face/{face}.v1"))),
        parameters,
        revision: legacy.revision,
    };
    let diagnostics = loadout.validate();
    if diagnostics.is_empty() {
        loadout.canonicalize();
        Ok(loadout)
    } else {
        Err(diagnostics)
    }
}

fn parse_known_id(value: &str) -> MorphAssetId {
    MorphAssetId::parse(value).expect("built-in compatibility ID must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_variants_become_one_base_with_independent_hair() {
        let girl = migrate_v1_appearance(&LegacyAppearance {
            body: Some("cuba:person-girl.v1".to_owned()),
            face: Some("curious".to_owned()),
            outfit: Some("cuba:everyday-hoodie.v1".to_owned()),
            colors: BTreeMap::from([("skin".to_owned(), "#efb083".to_owned())]),
            revision: 9,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(girl.base.as_str(), "cuba:base/person.v1");
        assert!(
            girl.parts
                .iter()
                .any(|part| part.as_str() == "cuba:hair/side-ponytail.v1")
        );
        assert!(
            girl.parts
                .iter()
                .any(|part| part.as_str() == "cuba:everyday-hoodie.v1")
        );
        assert_eq!(girl.face.unwrap().as_str(), "cuba:face/curious.v1");
        assert_eq!(girl.revision, 9);
    }

    #[test]
    fn creatures_keep_distinct_bases() {
        for (legacy, base) in [
            ("cuba:cat.v1", "cuba:base/cat.v1"),
            ("cuba:wolf.v1", "cuba:base/wolf.v1"),
            ("cuba:dragon.v1", "cuba:base/dragon.v1"),
        ] {
            let loadout = migrate_v1_appearance(&LegacyAppearance {
                body: Some(legacy.to_owned()),
                ..Default::default()
            })
            .unwrap();
            assert_eq!(loadout.base.as_str(), base);
        }
    }

    #[test]
    fn unknown_bodies_do_not_silently_change_identity() {
        let diagnostics = migrate_v1_appearance(&LegacyAppearance {
            body: Some("cuba:unknown.v1".to_owned()),
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(diagnostics[0].code, "MORPH_LEGACY_UNKNOWN_BODY");
    }
}
