use crate::{CapabilityId, MorphAssetId, MorphDiagnostic};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MORPH_CATALOG_SCHEMA_VERSION: u16 = 1;
pub const MAX_CATALOG_BYTES: usize = 1024 * 1024;
pub const MAX_CATALOG_ASSETS: usize = 512;
pub const MAX_CATALOG_PRESETS: usize = 256;
const MAX_DISPLAY_NAME_BYTES: usize = 96;
const MAX_TAG_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MorphAssetKind {
    Base,
    Face,
    Hair,
    Outfit,
    Top,
    Outerwear,
    Bottom,
    OnePiece,
    Footwear,
    Headwear,
    Facewear,
    Accessory,
    Tail,
    Wings,
    Horns,
    Ears,
    HeldItem,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphLodBudget {
    pub near: u32,
    pub mid: u32,
    pub far: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphProvenance {
    pub source: String,
    pub license: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphSourceReference {
    /// Relative path to an authored GLB/GLTF source. Runtime packs never load
    /// this path; it is for Studio reimport and provenance only.
    pub geometry: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphAssetDefinition {
    pub id: MorphAssetId,
    pub kind: MorphAssetKind,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rig_profile: Option<MorphAssetId>,
    pub fit_profiles: Vec<MorphAssetId>,
    #[serde(default)]
    pub supported_bases: Vec<MorphAssetId>,
    pub occupied_slots: Vec<String>,
    pub coverage: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    pub materials: Vec<String>,
    pub lod: MorphLodBudget,
    #[serde(default)]
    pub required_capabilities: Vec<CapabilityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<MorphSourceReference>,
    pub provenance: MorphProvenance,
}

impl MorphAssetDefinition {
    /// Validate one definition without requiring the caller to construct a
    /// complete catalog. Catalog-level reference checks remain the job of
    /// `MorphCatalog::validate`.
    pub fn validate(&self) -> Vec<MorphDiagnostic> {
        MorphCatalog {
            schema_version: MORPH_CATALOG_SCHEMA_VERSION,
            content_version: "definition-validation".to_owned(),
            assets: vec![self.clone()],
            presets: Vec::new(),
        }
        .validate()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphPreset {
    pub id: MorphAssetId,
    pub display_name: String,
    pub base: MorphAssetId,
    #[serde(default)]
    pub parts: Vec<MorphAssetId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphCatalog {
    pub schema_version: u16,
    pub content_version: String,
    pub assets: Vec<MorphAssetDefinition>,
    #[serde(default)]
    pub presets: Vec<MorphPreset>,
}

impl MorphCatalog {
    pub fn validate(&self) -> Vec<MorphDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.schema_version != MORPH_CATALOG_SCHEMA_VERSION {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_CATALOG_UNSUPPORTED_SCHEMA",
                "schemaVersion",
                format!("expected schema {MORPH_CATALOG_SCHEMA_VERSION}"),
            ));
        }
        if self.content_version.is_empty()
            || self.content_version.len() > MAX_DISPLAY_NAME_BYTES
            || !self.content_version.is_ascii()
        {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_CATALOG_INVALID_CONTENT_VERSION",
                "contentVersion",
                "content version must be 1..=96 ASCII bytes",
            ));
        }
        if self.assets.is_empty() || self.assets.len() > MAX_CATALOG_ASSETS {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_CATALOG_INVALID_ASSET_COUNT",
                "assets",
                format!("catalog must contain 1..={MAX_CATALOG_ASSETS} assets"),
            ));
        }
        if self.presets.len() > MAX_CATALOG_PRESETS {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_CATALOG_TOO_MANY_PRESETS",
                "presets",
                format!("at most {MAX_CATALOG_PRESETS} presets are allowed"),
            ));
        }

        let mut asset_ids = BTreeSet::new();
        for (index, asset) in self.assets.iter().enumerate() {
            let path = format!("assets[{index}]");
            validate_published_id(&asset.id, &format!("{path}.id"), &mut diagnostics);
            if !asset_ids.insert(&asset.id) {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_DUPLICATE_ASSET",
                    &format!("{path}.id"),
                    format!("asset {} is declared more than once", asset.id),
                ));
            }
            validate_display_name(
                &asset.display_name,
                &format!("{path}.displayName"),
                &mut diagnostics,
            );
            if let Some(rig_profile) = &asset.rig_profile {
                validate_published_id(rig_profile, &format!("{path}.rigProfile"), &mut diagnostics);
            }
            for (field, values) in [
                ("fitProfiles", &asset.fit_profiles),
                ("supportedBases", &asset.supported_bases),
            ] {
                for (value_index, value) in values.iter().enumerate() {
                    validate_published_id(
                        value,
                        &format!("{path}.{field}[{value_index}]"),
                        &mut diagnostics,
                    );
                }
            }
            for (field, values) in [
                ("occupiedSlots", &asset.occupied_slots),
                ("coverage", &asset.coverage),
                ("conflicts", &asset.conflicts),
                ("materials", &asset.materials),
            ] {
                validate_tags(values, &format!("{path}.{field}"), &mut diagnostics);
            }
            if asset.fit_profiles.is_empty() {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_MISSING_FIT_PROFILE",
                    &format!("{path}.fitProfiles"),
                    "every asset needs at least one fit profile",
                ));
            }
            if asset.occupied_slots.is_empty()
                || asset.coverage.is_empty()
                || asset.materials.is_empty()
            {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_MISSING_RENDER_METADATA",
                    &path,
                    "every asset needs slots, coverage, and material metadata",
                ));
            }
            if asset.lod.near < asset.lod.mid || asset.lod.mid < asset.lod.far || asset.lod.far == 0
            {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_INVALID_LOD",
                    &format!("{path}.lod"),
                    "LOD budgets must be near >= mid >= far > 0",
                ));
            }
            if asset.kind == MorphAssetKind::Base && !asset.supported_bases.is_empty() {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_BASE_HAS_SUPPORTED_BASES",
                    &format!("{path}.supportedBases"),
                    "base assets cannot list another supported base",
                ));
            }
            if asset.kind != MorphAssetKind::Base && asset.supported_bases.is_empty() {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_MISSING_SUPPORTED_BASE",
                    &format!("{path}.supportedBases"),
                    "non-base assets need at least one compatible base",
                ));
            }
            if let Some(source) = &asset.source {
                validate_source_path(
                    &source.geometry,
                    &format!("{path}.source.geometry"),
                    &mut diagnostics,
                );
            }
            if asset.provenance.source.is_empty() || asset.provenance.license.is_empty() {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_MISSING_PROVENANCE",
                    &format!("{path}.provenance"),
                    "every asset needs source and license provenance",
                ));
            }
        }

        let mut preset_ids = BTreeSet::new();
        for (index, preset) in self.presets.iter().enumerate() {
            let path = format!("presets[{index}]");
            validate_published_id(&preset.id, &format!("{path}.id"), &mut diagnostics);
            if !preset_ids.insert(&preset.id) {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_DUPLICATE_PRESET",
                    &format!("{path}.id"),
                    format!("preset {} is declared more than once", preset.id),
                ));
            }
            validate_display_name(
                &preset.display_name,
                &format!("{path}.displayName"),
                &mut diagnostics,
            );
            validate_reference(&preset.base, "base", &asset_ids, &path, &mut diagnostics);
            if let Some(base) = self.assets.iter().find(|asset| asset.id == preset.base)
                && base.kind != MorphAssetKind::Base
            {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_CATALOG_PRESET_BASE_NOT_BASE",
                    &format!("{path}.base"),
                    format!("{} is not a base asset", preset.base),
                ));
            }
            for (part_index, part) in preset.parts.iter().enumerate() {
                validate_reference(
                    part,
                    &format!("parts[{part_index}]"),
                    &asset_ids,
                    &path,
                    &mut diagnostics,
                );
                if let Some(asset) = self.assets.iter().find(|asset| asset.id == *part)
                    && !asset.supported_bases.is_empty()
                    && !asset.supported_bases.contains(&preset.base)
                {
                    diagnostics.push(MorphDiagnostic::error(
                        "MORPH_CATALOG_PRESET_UNSUPPORTED_PART",
                        &format!("{path}.parts[{part_index}]"),
                        format!("{} does not support base {}", part, preset.base),
                    ));
                }
            }
        }
        diagnostics
    }

    pub fn asset(&self, id: &MorphAssetId) -> Option<&MorphAssetDefinition> {
        self.assets.iter().find(|asset| asset.id == *id)
    }
}

pub fn parse_catalog(source: &str) -> Result<MorphCatalog, Vec<MorphDiagnostic>> {
    if source.len() > MAX_CATALOG_BYTES {
        return Err(vec![MorphDiagnostic::error(
            "MORPH_CATALOG_TOO_LARGE",
            "$",
            format!("catalog exceeds {MAX_CATALOG_BYTES} bytes"),
        )]);
    }
    let catalog: MorphCatalog = serde_json::from_str(source).map_err(|error| {
        vec![MorphDiagnostic::error(
            "MORPH_CATALOG_INVALID_JSON",
            "$",
            error.to_string(),
        )]
    })?;
    let diagnostics = catalog.validate();
    if diagnostics.is_empty() {
        Ok(catalog)
    } else {
        Err(diagnostics)
    }
}

fn validate_published_id(id: &MorphAssetId, path: &str, diagnostics: &mut Vec<MorphDiagnostic>) {
    if let Err(error) = id.validate_published() {
        diagnostics.push(MorphDiagnostic::error(
            "MORPH_CATALOG_INVALID_ASSET_ID",
            path,
            error.to_string(),
        ));
    }
}

fn validate_display_name(value: &str, path: &str, diagnostics: &mut Vec<MorphDiagnostic>) {
    if value.is_empty() || value.len() > MAX_DISPLAY_NAME_BYTES || !value.is_ascii() {
        diagnostics.push(MorphDiagnostic::error(
            "MORPH_CATALOG_INVALID_DISPLAY_NAME",
            path,
            "display names must be 1..=96 ASCII bytes",
        ));
    }
}

fn validate_tags(values: &[String], path: &str, diagnostics: &mut Vec<MorphDiagnostic>) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.is_empty()
            || value.len() > MAX_TAG_BYTES
            || !value.is_ascii()
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
            })
        {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_CATALOG_INVALID_TAG",
                &format!("{path}[{index}]"),
                "tags must be 1..=64 lower-case ASCII bytes",
            ));
        }
        if !seen.insert(value) {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_CATALOG_DUPLICATE_TAG",
                &format!("{path}[{index}]"),
                format!("tag {value:?} is repeated"),
            ));
        }
    }
}

fn validate_source_path(value: &str, path: &str, diagnostics: &mut Vec<MorphDiagnostic>) {
    let valid = !value.is_empty()
        && value.len() <= 240
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    if !valid || !(value.ends_with(".glb") || value.ends_with(".gltf")) {
        diagnostics.push(MorphDiagnostic::error(
            "MORPH_CATALOG_INVALID_SOURCE_PATH",
            path,
            "geometry must be a safe relative .glb or .gltf path",
        ));
    }
}

fn validate_reference(
    id: &MorphAssetId,
    field: &str,
    known: &BTreeSet<&MorphAssetId>,
    path: &str,
    diagnostics: &mut Vec<MorphDiagnostic>,
) {
    if !known.contains(id) {
        diagnostics.push(MorphDiagnostic::error(
            "MORPH_CATALOG_UNKNOWN_REFERENCE",
            &format!("{path}.{field}"),
            format!("catalog does not contain {id}"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../assets/characters/morph_catalog.json");

    #[test]
    fn bundled_compatibility_catalog_is_valid_and_inspectable() {
        let catalog = parse_catalog(FIXTURE).expect("morph catalog");
        assert_eq!(catalog.assets.len(), 34);
        assert_eq!(catalog.presets.len(), 3);
        let person = MorphAssetId::parse("cuba:base/person.v1").unwrap();
        assert_eq!(catalog.asset(&person).unwrap().kind, MorphAssetKind::Base);
    }

    #[test]
    fn rejects_unknown_preset_parts() {
        let mut catalog = parse_catalog(FIXTURE).unwrap();
        catalog.presets[0]
            .parts
            .push(MorphAssetId::parse("cuba:hat/missing.v1").unwrap());
        assert!(
            catalog
                .validate()
                .iter()
                .any(|diagnostic| diagnostic.code == "MORPH_CATALOG_UNKNOWN_REFERENCE")
        );
    }

    #[test]
    fn rejects_unsafe_geometry_paths() {
        let mut catalog = parse_catalog(FIXTURE).unwrap();
        catalog.assets[0].source = Some(MorphSourceReference {
            geometry: "../outside.glb".to_owned(),
        });
        assert!(
            catalog
                .validate()
                .iter()
                .any(|diagnostic| diagnostic.code == "MORPH_CATALOG_INVALID_SOURCE_PATH")
        );
    }
}
