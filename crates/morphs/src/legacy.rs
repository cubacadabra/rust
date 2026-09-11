use crate::{
    MorphAssetId, MorphAssetKind, MorphCatalog, MorphDiagnostic, MorphLoadout, MorphParameterValue,
};
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
    let (base_name, hair) = match body {
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
    let mut base = parse_known_id(base_name);
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
            Ok(item) if slot == "base" && item.as_str().starts_with("cuba:base/") => {
                base = item;
            }
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
        base,
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

/// Project a validated V2 loadout into the legacy appearance wire object used
/// by the current procedural renderer. This is deliberately a compatibility
/// boundary: Studio and clients can speak one loadout format while the old
/// renderer is replaced asset-by-asset with GLB capabilities.
pub fn project_v2_to_v1(
    catalog: &MorphCatalog,
    loadout: &MorphLoadout,
) -> Result<LegacyAppearance, Vec<MorphDiagnostic>> {
    let mut diagnostics = loadout.validate();
    let Some(base) = catalog.asset(&loadout.base) else {
        diagnostics.push(MorphDiagnostic::error(
            "MORPH_LEGACY_UNKNOWN_BASE",
            "base",
            format!("catalog does not contain {}", loadout.base),
        ));
        return Err(diagnostics);
    };
    if base.kind != MorphAssetKind::Base {
        diagnostics.push(MorphDiagnostic::error(
            "MORPH_LEGACY_INVALID_BASE",
            "base",
            format!("{} is not a base asset", loadout.base),
        ));
    }

    let body = match loadout.base.as_str() {
        "cuba:base/person.v1" => "cuba:person.v1",
        "cuba:base/person-authored.v1" => "cuba:person.v1",
        "cuba:base/cat.v1" => "cuba:cat.v1",
        "cuba:base/wolf.v1" => "cuba:wolf.v1",
        "cuba:base/dragon.v1" => "cuba:dragon.v1",
        _ => {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_LEGACY_UNSUPPORTED_BASE",
                "base",
                format!("{} has no legacy renderer projection", loadout.base),
            ));
            "cuba:person.v1"
        }
    };

    let mut legacy_body = body;
    let mut outfit = None;
    let mut equipment = BTreeMap::new();
    for (index, part_id) in loadout.parts.iter().enumerate() {
        let path = format!("parts[{index}]");
        let Some(part) = catalog.asset(part_id) else {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_LEGACY_UNKNOWN_PART",
                &path,
                format!("catalog does not contain {part_id}"),
            ));
            continue;
        };
        match part.kind {
            MorphAssetKind::Hair => match part_id.as_str() {
                "cuba:hair/swept.v1" => {}
                "cuba:hair/side-ponytail.v1" => legacy_body = "cuba:person-girl.v1",
                "cuba:hair/shag.v1" => legacy_body = "cuba:person-nb.v1",
                _ => diagnostics.push(MorphDiagnostic::error(
                    "MORPH_LEGACY_UNSUPPORTED_HAIR",
                    &path,
                    format!("{} has no legacy renderer projection", part_id),
                )),
            },
            MorphAssetKind::Outfit => {
                if outfit.is_some() {
                    diagnostics.push(MorphDiagnostic::error(
                        "MORPH_LEGACY_MULTIPLE_OUTFITS",
                        &path,
                        "the legacy renderer accepts one outfit",
                    ));
                } else {
                    outfit = Some(part_id.to_string());
                }
            }
            _ => {
                let slot = legacy_slot(part);
                if equipment
                    .insert(slot.to_owned(), part_id.to_string())
                    .is_some()
                {
                    diagnostics.push(MorphDiagnostic::error(
                        "MORPH_LEGACY_OCCUPIED_SLOT",
                        &path,
                        format!("multiple parts target legacy slot {slot:?}"),
                    ));
                }
            }
        }
    }

    let face = loadout
        .face
        .as_ref()
        .and_then(|id| id.as_str().strip_prefix("cuba:face/"))
        .and_then(|id| id.strip_suffix(".v1"))
        .map(ToOwned::to_owned);
    if loadout.face.is_some() && face.is_none() {
        diagnostics.push(MorphDiagnostic::error(
            "MORPH_LEGACY_UNSUPPORTED_FACE",
            "face",
            "face must use the V1 analytic face asset namespace",
        ));
    }

    let colors = loadout
        .parameters
        .iter()
        .filter_map(|(key, value)| match value {
            MorphParameterValue::Text(value) => Some((key.clone(), value.clone())),
            MorphParameterValue::Number(value) => Some((key.clone(), value.to_string())),
            MorphParameterValue::Boolean(value) => Some((key.clone(), value.to_string())),
        })
        .collect();
    if diagnostics.is_empty() {
        Ok(LegacyAppearance {
            version: Some(1),
            body: Some(legacy_body.to_owned()),
            face,
            outfit,
            equipment,
            colors,
            revision: loadout.revision,
        })
    } else {
        Err(diagnostics)
    }
}

fn legacy_slot(asset: &crate::MorphAssetDefinition) -> &'static str {
    if asset
        .occupied_slots
        .iter()
        .any(|slot| slot == "ear-accessory")
    {
        "ear-accessory"
    } else if asset.occupied_slots.iter().any(|slot| slot == "facewear") {
        "glasses"
    } else if asset.occupied_slots.iter().any(|slot| slot == "neck") {
        "neck"
    } else if asset.occupied_slots.iter().any(|slot| slot == "back") {
        "back"
    } else if asset.occupied_slots.iter().any(|slot| slot == "waist") {
        "waist"
    } else {
        "hat"
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

    #[test]
    fn projects_unified_loadout_for_the_current_renderer() {
        let catalog = crate::parse_catalog(include_str!(
            "../../../assets/characters/morph_catalog.json"
        ))
        .unwrap();
        let mut catalog = catalog;
        catalog.assets.push(crate::MorphAssetDefinition {
            id: parse_known_id("cuba:headphones.v1"),
            kind: MorphAssetKind::Headwear,
            display_name: "Headphones".to_owned(),
            rig_profile: Some(parse_known_id("cuba:rig/biped15.v1")),
            fit_profiles: vec![parse_known_id("cuba:fit/person-standard.v1")],
            supported_bases: vec![parse_known_id("cuba:base/person.v1")],
            occupied_slots: vec!["ear-accessory".to_owned()],
            coverage: vec!["ears".to_owned()],
            conflicts: Vec::new(),
            materials: vec!["default".to_owned()],
            lod: crate::MorphLodBudget {
                near: 3,
                mid: 2,
                far: 1,
            },
            required_capabilities: Vec::new(),
            source: None,
            provenance: crate::MorphProvenance {
                source: "test".to_owned(),
                license: "test".to_owned(),
            },
        });
        let loadout = MorphLoadout {
            version: 2,
            base: parse_known_id("cuba:base/person.v1"),
            parts: vec![
                parse_known_id("cuba:hair/side-ponytail.v1"),
                parse_known_id("cuba:everyday-hoodie.v1"),
                parse_known_id("cuba:headphones.v1"),
            ],
            face: Some(parse_known_id("cuba:face/happy.v1")),
            parameters: BTreeMap::new(),
            revision: 4,
        };
        let legacy = project_v2_to_v1(&catalog, &loadout).unwrap();
        assert_eq!(legacy.body.as_deref(), Some("cuba:person-girl.v1"));
        assert_eq!(legacy.outfit.as_deref(), Some("cuba:everyday-hoodie.v1"));
        assert_eq!(
            legacy.equipment.get("ear-accessory").unwrap(),
            "cuba:headphones.v1"
        );
        assert_eq!(legacy.revision, 4);
    }
}
