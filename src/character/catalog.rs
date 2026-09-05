//! Offline validation for the bounded Phase 5 procedural character catalog.

#![allow(dead_code)]

use super::{BodyId, OutfitId};
use super::definition::EquipmentSlot;
use serde::Deserialize;

const MAX_TEXTURE_PAYLOAD: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogFile {
    schema_version: u16,
    outfits: Vec<OutfitFile>,
    materials: Vec<MaterialFile>,
    #[serde(default)]
    licenses: Vec<LicenseFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutfitFile {
    id: String,
    supported_bodies: Vec<String>,
    occupied_slots: Vec<String>,
    coverage: Vec<String>,
    conflicts: Vec<String>,
    materials: Vec<String>,
    lod: LodFile,
    #[serde(default)]
    texture_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LodFile {
    near: u32,
    mid: u32,
    far: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct MaterialFile {
    id: String,
    #[serde(default)]
    texture: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct LicenseFile {
    id: String,
    source: String,
    license: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogValidationReport {
    pub schema_version: u16,
    pub outfit_count: usize,
    pub material_count: usize,
    pub texture_bytes: u64,
    pub license_count: usize,
}

pub(crate) fn validate_catalog(source: &str) -> Result<CatalogValidationReport, String> {
    let catalog: CatalogFile = serde_json::from_str(source)
        .map_err(|error| format!("catalog JSON is invalid: {error}"))?;
    if catalog.schema_version != 1 {
        return Err(format!(
            "unsupported catalog schema {}",
            catalog.schema_version
        ));
    }
    if catalog.outfits.len() != OutfitId::ALL.len() {
        return Err(format!(
            "catalog must contain exactly {} outfits",
            OutfitId::ALL.len()
        ));
    }
    let mut texture_bytes = 0_u64;
    for outfit in &catalog.outfits {
        let id = OutfitId::from_stable_id(&outfit.id)
            .ok_or_else(|| format!("unknown outfit id {:?}", outfit.id))?;
        if outfit.supported_bodies.is_empty()
            || outfit.occupied_slots.is_empty()
            || outfit.coverage.is_empty()
            || outfit.materials.is_empty()
            || outfit.lod.near < outfit.lod.mid
            || outfit.lod.mid < outfit.lod.far
            || outfit.lod.far == 0
        {
            return Err(format!(
                "outfit {:?} has invalid fit/LOD metadata",
                outfit.id
            ));
        }
        if id.material_family().is_empty()
            || outfit
                .occupied_slots
                .iter()
                .any(|slot| slot.is_empty())
            || outfit.conflicts.iter().any(String::is_empty)
        {
            return Err(format!("outfit {:?} has empty slot/material metadata", outfit.id));
        }
        for body in &outfit.supported_bodies {
            let body_id = BodyId::from_stable_id(body)
                .ok_or_else(|| format!("outfit {:?} has unknown body {:?}", outfit.id, body))?;
            if !id.supported_by(body_id) {
                return Err(format!(
                    "outfit {:?} claims unsupported body {:?}",
                    outfit.id, body
                ));
            }
        }
        texture_bytes = texture_bytes.saturating_add(outfit.texture_bytes);
    }
    if texture_bytes > MAX_TEXTURE_PAYLOAD {
        return Err(format!(
            "catalog texture payload exceeds {MAX_TEXTURE_PAYLOAD} bytes"
        ));
    }
    if EquipmentSlot::ALL.is_empty() {
        return Err("equipment slot vocabulary cannot be empty".to_owned());
    }
    for material in &catalog.materials {
        if material.id.is_empty() || material.id.len() > 96 {
            return Err("material IDs must be non-empty and <= 96 bytes".to_owned());
        }
        if let Some(texture) = &material.texture {
            if texture.is_empty() {
                return Err(format!(
                    "material {:?} has an empty texture path",
                    material.id
                ));
            }
        }
    }
    for license in &catalog.licenses {
        if license.id.is_empty() || license.source.is_empty() || license.license.is_empty() {
            return Err("every catalog asset needs source/license provenance".to_owned());
        }
    }
    Ok(CatalogValidationReport {
        schema_version: catalog.schema_version,
        outfit_count: catalog.outfits.len(),
        material_count: catalog.materials.len(),
        texture_bytes,
        license_count: catalog.licenses.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../assets/characters/catalog.json");

    #[test]
    fn bundled_phase5_catalog_is_complete_and_bounded() {
        let report = validate_catalog(FIXTURE).expect("bundled character catalog");
        assert_eq!(report.outfit_count, OutfitId::ALL.len());
        assert!(report.material_count >= 8);
        assert!(report.license_count >= report.outfit_count);
        assert!(report.texture_bytes <= MAX_TEXTURE_PAYLOAD);
    }

    #[test]
    fn catalog_rejects_unknown_body_fit() {
        let source = FIXTURE.replace("cuba:cat.v1", "cuba:person.v1");
        assert!(validate_catalog(&source).is_err());
    }
}
