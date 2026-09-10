use crate::{MorphAssetId, MorphDiagnostic};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MORPH_LOADOUT_VERSION: u16 = 2;
pub const MAX_LOADOUT_BYTES: usize = 4096;
pub const MAX_PARTS: usize = 32;
pub const MAX_PARAMETERS: usize = 16;
const MAX_PARAMETER_KEY_BYTES: usize = 64;
const MAX_PARAMETER_TEXT_BYTES: usize = 96;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MorphParameterValue {
    Text(String),
    Number(f32),
    Boolean(bool),
}

/// Compact, network-safe selection data. Geometry and textures live in a
/// separately validated morph pack and are referenced only by stable IDs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphLoadout {
    pub version: u16,
    pub base: MorphAssetId,
    #[serde(default)]
    pub parts: Vec<MorphAssetId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<MorphAssetId>,
    #[serde(default)]
    pub parameters: BTreeMap<String, MorphParameterValue>,
    #[serde(default)]
    pub revision: u32,
}

impl MorphLoadout {
    pub fn validate(&self) -> Vec<MorphDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.version != MORPH_LOADOUT_VERSION {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_LOADOUT_UNSUPPORTED_VERSION",
                "version",
                format!("expected version {MORPH_LOADOUT_VERSION}"),
            ));
        }
        validate_published_id(&self.base, "base", &mut diagnostics);
        if self.parts.len() > MAX_PARTS {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_LOADOUT_TOO_MANY_PARTS",
                "parts",
                format!("at most {MAX_PARTS} parts are allowed"),
            ));
        }
        let mut seen = BTreeSet::new();
        for (index, part) in self.parts.iter().enumerate() {
            let path = format!("parts[{index}]");
            validate_published_id(part, &path, &mut diagnostics);
            if !seen.insert(part) {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_LOADOUT_DUPLICATE_PART",
                    &path,
                    format!("part {part} is selected more than once"),
                ));
            }
        }
        if let Some(face) = &self.face {
            validate_published_id(face, "face", &mut diagnostics);
        }
        if self.parameters.len() > MAX_PARAMETERS {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_LOADOUT_TOO_MANY_PARAMETERS",
                "parameters",
                format!("at most {MAX_PARAMETERS} parameters are allowed"),
            ));
        }
        for (key, value) in &self.parameters {
            let path = format!("parameters.{key}");
            if key.is_empty()
                || key.len() > MAX_PARAMETER_KEY_BYTES
                || !key.is_ascii()
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
            {
                diagnostics.push(MorphDiagnostic::error(
                    "MORPH_LOADOUT_INVALID_PARAMETER",
                    &path,
                    "parameter names must be 1..=64 safe ASCII bytes",
                ));
            }
            match value {
                MorphParameterValue::Text(text)
                    if text.len() > MAX_PARAMETER_TEXT_BYTES || !text.is_ascii() =>
                {
                    diagnostics.push(MorphDiagnostic::error(
                        "MORPH_LOADOUT_INVALID_PARAMETER",
                        &path,
                        "text parameters must be at most 96 ASCII bytes",
                    ));
                }
                MorphParameterValue::Number(number) if !number.is_finite() => {
                    diagnostics.push(MorphDiagnostic::error(
                        "MORPH_LOADOUT_INVALID_PARAMETER",
                        &path,
                        "numeric parameters must be finite",
                    ));
                }
                _ => {}
            }
        }
        if serde_json::to_vec(self).is_ok_and(|source| source.len() > MAX_LOADOUT_BYTES) {
            diagnostics.push(MorphDiagnostic::error(
                "MORPH_LOADOUT_TOO_LARGE",
                "$",
                format!("serialized loadout exceeds {MAX_LOADOUT_BYTES} bytes"),
            ));
        }
        diagnostics
    }

    /// Part order has no appearance meaning. Canonical order makes hashes,
    /// diffs, persistence, and multiplayer comparison deterministic.
    pub fn canonicalize(&mut self) {
        self.parts.sort();
    }
}

pub fn parse_loadout(source: &str) -> Result<MorphLoadout, Vec<MorphDiagnostic>> {
    if source.len() > MAX_LOADOUT_BYTES {
        return Err(vec![MorphDiagnostic::error(
            "MORPH_LOADOUT_TOO_LARGE",
            "$",
            format!("loadout exceeds {MAX_LOADOUT_BYTES} bytes"),
        )]);
    }
    let mut loadout: MorphLoadout = serde_json::from_str(source).map_err(|error| {
        vec![MorphDiagnostic::error(
            "MORPH_LOADOUT_INVALID_JSON",
            "$",
            error.to_string(),
        )]
    })?;
    let diagnostics = loadout.validate();
    if diagnostics.is_empty() {
        loadout.canonicalize();
        Ok(loadout)
    } else {
        Err(diagnostics)
    }
}

fn validate_published_id(id: &MorphAssetId, path: &str, diagnostics: &mut Vec<MorphDiagnostic>) {
    if let Err(error) = id.validate_published() {
        diagnostics.push(MorphDiagnostic::error(
            "MORPH_LOADOUT_INVALID_ASSET_ID",
            path,
            error.to_string(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_canonicalizes_a_bounded_loadout() {
        let source = r##"{
            "version": 2,
            "base": "cuba:base/person.v1",
            "parts": ["cuba:top/hoodie.v1", "cuba:hair/swept.v1"],
            "face": "cuba:face/happy.v1",
            "parameters": {"skin": "#e8ae86", "roughness": 0.7},
            "revision": 17
        }"##;
        let loadout = parse_loadout(source).unwrap();
        assert_eq!(loadout.parts[0].as_str(), "cuba:hair/swept.v1");
        assert_eq!(loadout.parts[1].as_str(), "cuba:top/hoodie.v1");
        assert_eq!(loadout.revision, 17);
    }

    #[test]
    fn reports_stable_paths_for_invalid_content() {
        let source = r#"{
            "version": 1,
            "base": "draft_base",
            "parts": ["cuba:hat/star.v1", "cuba:hat/star.v1"]
        }"#;
        let diagnostics = parse_loadout(source).unwrap_err();
        assert!(diagnostics.iter().any(|item| {
            item.code == "MORPH_LOADOUT_UNSUPPORTED_VERSION" && item.path == "version"
        }));
        assert!(
            diagnostics.iter().any(|item| {
                item.code == "MORPH_LOADOUT_INVALID_ASSET_ID" && item.path == "base"
            })
        );
        assert!(diagnostics.iter().any(|item| {
            item.code == "MORPH_LOADOUT_DUPLICATE_PART" && item.path == "parts[1]"
        }));
    }

    #[test]
    fn rejects_an_oversized_wire_payload_before_parsing() {
        let source = "x".repeat(MAX_LOADOUT_BYTES + 1);
        let diagnostics = parse_loadout(&source).unwrap_err();
        assert_eq!(diagnostics[0].code, "MORPH_LOADOUT_TOO_LARGE");
    }
}
