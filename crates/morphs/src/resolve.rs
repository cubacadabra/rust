use crate::{
    CapabilityId, CapabilitySet, MorphAssetDefinition, MorphAssetId, MorphAssetKind, MorphCatalog,
    MorphDiagnostic, MorphLoadout,
};
use std::collections::BTreeSet;

/// The result of resolving a network-safe loadout against one catalog and one
/// engine capability set. It contains references only; meshes remain in the
/// compiled morph pack selected by the runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedMorphLoadout {
    pub base: MorphAssetId,
    pub parts: Vec<MorphAssetId>,
    pub face: Option<MorphAssetId>,
    pub fit_profile: MorphAssetId,
    pub required_capabilities: Vec<CapabilityId>,
}

/// Resolve a loadout without mutating it or the catalog. Diagnostics are
/// deterministic so Studio, runtime clients, and tests report the same issue
/// ordering for the same content.
pub fn resolve_loadout(
    catalog: &MorphCatalog,
    loadout: &MorphLoadout,
    capabilities: &CapabilitySet,
) -> Result<ResolvedMorphLoadout, Vec<MorphDiagnostic>> {
    let mut diagnostics = catalog.validate();
    diagnostics.extend(loadout.validate());
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut normalized = loadout.clone();
    normalized.canonicalize();
    let Some(base) = catalog.asset(&normalized.base) else {
        return Err(vec![error(
            "MORPH_RESOLVE_UNKNOWN_BASE",
            "base",
            format!("catalog does not contain {}", normalized.base),
        )]);
    };
    if base.kind != MorphAssetKind::Base {
        return Err(vec![error(
            "MORPH_RESOLVE_BASE_NOT_BASE",
            "base",
            format!("{} is not a base asset", normalized.base),
        )]);
    }

    let mut selected =
        Vec::with_capacity(normalized.parts.len() + usize::from(normalized.face.is_some()));
    for (index, id) in normalized.parts.iter().enumerate() {
        let path = format!("parts[{index}]");
        match catalog.asset(id) {
            Some(asset) => selected.push((path, id, asset)),
            None => diagnostics.push(error(
                "MORPH_RESOLVE_UNKNOWN_PART",
                &path,
                format!("catalog does not contain {id}"),
            )),
        }
    }
    if let Some(face_id) = &normalized.face {
        let path = "face".to_owned();
        match catalog.asset(face_id) {
            Some(asset) => {
                if asset.kind != MorphAssetKind::Face {
                    diagnostics.push(error(
                        "MORPH_RESOLVE_FACE_NOT_FACE",
                        &path,
                        format!("{face_id} is not a face asset"),
                    ));
                }
                selected.push((path, face_id, asset));
            }
            None => diagnostics.push(error(
                "MORPH_RESOLVE_UNKNOWN_FACE",
                &path,
                format!("catalog does not contain {face_id}"),
            )),
        }
    }

    let mut fit_profiles: BTreeSet<MorphAssetId> = base.fit_profiles.iter().cloned().collect();
    let mut required_capabilities = BTreeSet::new();
    required_capabilities.extend(base.required_capabilities.iter().cloned());
    for (path, id, asset) in &selected {
        if asset.kind == MorphAssetKind::Base {
            diagnostics.push(error(
                "MORPH_RESOLVE_BASE_AS_PART",
                path,
                format!("{id} cannot be selected as a part"),
            ));
        }
        if !asset.supported_bases.contains(&normalized.base) {
            diagnostics.push(error(
                "MORPH_RESOLVE_UNSUPPORTED_BASE",
                path,
                format!("{id} does not support base {}", normalized.base),
            ));
        }
        if let (Some(base_rig), Some(part_rig)) = (&base.rig_profile, &asset.rig_profile)
            && base_rig != part_rig
        {
            diagnostics.push(error(
                "MORPH_RESOLVE_RIG_MISMATCH",
                path,
                format!("{id} targets rig {part_rig}, base uses {base_rig}"),
            ));
        }
        fit_profiles.retain(|fit| asset.fit_profiles.contains(fit));
        required_capabilities.extend(asset.required_capabilities.iter().cloned());
    }
    if fit_profiles.is_empty() {
        diagnostics.push(error(
            "MORPH_RESOLVE_NO_COMMON_FIT",
            "parts",
            "selected assets have no common fit profile",
        ));
    }

    validate_slots_and_conflicts(&selected, &mut diagnostics);
    let required_capabilities = required_capabilities.into_iter().collect::<Vec<_>>();
    for capability in capabilities.missing(&required_capabilities) {
        diagnostics.push(error(
            "MORPH_RESOLVE_MISSING_CAPABILITY",
            "capabilities",
            format!("engine does not support {capability}"),
        ));
    }

    if diagnostics.is_empty() {
        Ok(ResolvedMorphLoadout {
            base: normalized.base,
            parts: normalized.parts,
            face: normalized.face,
            fit_profile: fit_profiles
                .into_iter()
                .next()
                .expect("a valid resolution has a common fit profile"),
            required_capabilities,
        })
    } else {
        Err(diagnostics)
    }
}

/// Resolve one catalog preset through the same loadout path used by network
/// appearances. Presets are content conveniences, not a second resolver.
pub fn resolve_preset(
    catalog: &MorphCatalog,
    preset_id: &MorphAssetId,
    capabilities: &CapabilitySet,
) -> Result<ResolvedMorphLoadout, Vec<MorphDiagnostic>> {
    let Some(preset) = catalog
        .presets
        .iter()
        .find(|preset| preset.id == *preset_id)
    else {
        return Err(vec![error(
            "MORPH_RESOLVE_UNKNOWN_PRESET",
            "preset",
            format!("catalog does not contain {preset_id}"),
        )]);
    };
    let loadout = preset.loadout();
    resolve_loadout(catalog, &loadout, capabilities)
}

fn validate_slots_and_conflicts(
    selected: &[(String, &MorphAssetId, &MorphAssetDefinition)],
    diagnostics: &mut Vec<MorphDiagnostic>,
) {
    for left_index in 0..selected.len() {
        let (left_path, left_id, left) = &selected[left_index];
        for (right_path, right_id, right) in selected.iter().skip(left_index + 1) {
            for slot in left
                .occupied_slots
                .iter()
                .filter(|slot| right.occupied_slots.contains(slot))
            {
                diagnostics.push(error(
                    "MORPH_RESOLVE_OCCUPIED_SLOT",
                    right_path,
                    format!("{right_id} shares occupied slot {slot:?} with {left_id}"),
                ));
            }
            for conflict in left
                .conflicts
                .iter()
                .filter(|conflict| right.occupied_slots.contains(conflict))
            {
                diagnostics.push(error(
                    "MORPH_RESOLVE_CONFLICT_TAG",
                    right_path,
                    format!("{right_id} conflicts with {left_id} through tag {conflict:?}"),
                ));
            }
            for conflict in right
                .conflicts
                .iter()
                .filter(|conflict| left.occupied_slots.contains(conflict))
            {
                diagnostics.push(error(
                    "MORPH_RESOLVE_CONFLICT_TAG",
                    left_path,
                    format!("{left_id} conflicts with {right_id} through tag {conflict:?}"),
                ));
            }
        }
    }
}

fn error(code: &str, path: &str, message: impl Into<String>) -> MorphDiagnostic {
    MorphDiagnostic::error(code, path, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityId, LegacyAppearance, MORPH_LOADOUT_VERSION, migrate_v1_appearance, parse_catalog,
    };

    const FIXTURE: &str = include_str!("../../../assets/characters/morph_catalog.json");

    fn all_capabilities() -> CapabilitySet {
        CapabilitySet::new([
            CapabilityId::parse("mesh.rigid.v1").unwrap(),
            CapabilityId::parse("face.analytic.v1").unwrap(),
            CapabilityId::parse("secondary.chain.v1").unwrap(),
            CapabilityId::parse("material.emissive.v1").unwrap(),
            CapabilityId::parse("skin.biped15-linear.v1").unwrap(),
            CapabilityId::parse("material.cuba-pbr.v1").unwrap(),
        ])
    }

    #[test]
    fn resolves_legacy_person_preset_with_common_fit_and_capabilities() {
        let catalog = parse_catalog(FIXTURE).unwrap();
        let preset = MorphAssetId::parse("cuba:preset/person-01.v1").unwrap();
        let resolved = resolve_preset(&catalog, &preset, &all_capabilities()).unwrap();
        assert_eq!(resolved.base.as_str(), "cuba:base/person.v1");
        assert_eq!(resolved.fit_profile.as_str(), "cuba:fit/person-standard.v1");
        assert_eq!(resolved.parts.len(), 2);
        assert!(
            resolved
                .required_capabilities
                .iter()
                .any(|capability| { capability.as_str() == "secondary.chain.v1" })
        );
    }

    #[test]
    fn reports_unsupported_base_and_missing_capability() {
        let mut catalog = parse_catalog(FIXTURE).unwrap();
        let hair = MorphAssetId::parse("cuba:hair/side-ponytail.v1").unwrap();
        catalog
            .assets
            .iter_mut()
            .find(|asset| asset.id == hair)
            .unwrap()
            .supported_bases
            .retain(|base| base.as_str() != "cuba:base/person-02.v1");
        let loadout = MorphLoadout {
            version: MORPH_LOADOUT_VERSION,
            base: MorphAssetId::parse("cuba:base/person-02.v1").unwrap(),
            parts: vec![MorphAssetId::parse("cuba:hair/side-ponytail.v1").unwrap()],
            face: None,
            parameters: Default::default(),
            revision: 0,
        };
        let diagnostics = resolve_loadout(
            &catalog,
            &loadout,
            &CapabilitySet::new([CapabilityId::parse("mesh.rigid.v1").unwrap()]),
        )
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "MORPH_RESOLVE_UNSUPPORTED_BASE" })
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "MORPH_RESOLVE_MISSING_CAPABILITY"
                && diagnostic.message.contains("secondary.chain.v1")
        }));
    }

    #[test]
    fn reports_conflicting_occupied_slots_and_tags() {
        let catalog = parse_catalog(FIXTURE).unwrap();
        let loadout = MorphLoadout {
            version: MORPH_LOADOUT_VERSION,
            base: MorphAssetId::parse("cuba:base/person.v1").unwrap(),
            parts: vec![
                MorphAssetId::parse("cuba:hair/side-ponytail.v1").unwrap(),
                MorphAssetId::parse("cuba:glossy-raincoat.v1").unwrap(),
            ],
            face: None,
            parameters: Default::default(),
            revision: 0,
        };
        let diagnostics = resolve_loadout(&catalog, &loadout, &all_capabilities()).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "MORPH_RESOLVE_CONFLICT_TAG" })
        );
    }

    #[test]
    fn resolves_a_migrated_v1_appearance_including_its_face_asset() {
        let catalog = parse_catalog(FIXTURE).unwrap();
        let legacy = LegacyAppearance {
            body: Some("cuba:person-girl.v1".to_owned()),
            face: Some("curious".to_owned()),
            outfit: Some("cuba:everyday-hoodie.v1".to_owned()),
            ..Default::default()
        };
        let loadout = migrate_v1_appearance(&legacy).unwrap();
        let resolved = resolve_loadout(&catalog, &loadout, &all_capabilities()).unwrap();
        assert_eq!(resolved.face.unwrap().as_str(), "cuba:face/curious.v1");
        assert_eq!(resolved.parts.len(), 2);
    }
}
