use crate::{MorphAssetDefinition, MorphDiagnostic};
use serde::Deserialize;

pub const MORPH_PACK_SCHEMA_VERSION: u16 = 1;
pub const MORPH_PACK_SKINNED_SCHEMA_VERSION: u16 = 2;
pub const MORPH_PACK_MAGIC: &[u8; 8] = b"CUBAMORP";
pub const MAX_MORPH_PACK_BYTES: usize = 64 * 1024 * 1024;
const MAX_MORPH_PACK_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_MORPH_PACK_VERTICES: usize = 200_000;
const MAX_MORPH_PACK_INDICES: usize = 600_000;
const MAX_ATTACHED_VERTEX_ABS: f32 = 100.0;

#[derive(Clone, Debug, PartialEq)]
pub struct MorphPack {
    pub asset: MorphAssetDefinition,
    pub attachment: MorphPackAttachment,
    pub lods: [MorphPackLod; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct MorphPackAttachment {
    pub mode: MorphPackAttachmentMode,
    pub joint: String,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MorphPackAttachmentMode {
    Rigid,
    Skinned,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MorphPackLod {
    pub triangle_count: u32,
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub base_color: Option<[f32; 4]>,
    pub skinning: Option<Vec<MorphPackVertexSkin>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphPackVertexSkin {
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MorphPackManifest {
    asset: MorphAssetDefinition,
    attachment: MorphPackManifestAttachment,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MorphPackManifestAttachment {
    mode: String,
    joint: String,
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}

/// Decode a bounded, renderer-neutral compiled morph pack.
///
/// Runtime clients receive only this constrained format. They never parse
/// Blender/glTF data or run Studio authoring code.
pub fn decode_morph_pack(bytes: &[u8]) -> Result<MorphPack, Vec<MorphDiagnostic>> {
    if bytes.len() > MAX_MORPH_PACK_BYTES {
        return Err(vec![error(
            "MORPH_PACK_TOO_LARGE",
            "$",
            format!("pack exceeds {MAX_MORPH_PACK_BYTES} bytes"),
        )]);
    }
    let mut cursor = Cursor::new(bytes);
    if cursor.read_bytes(8, "magic")? != MORPH_PACK_MAGIC {
        return Err(vec![error(
            "MORPH_PACK_INVALID_MAGIC",
            "magic",
            "pack must begin with the CUBAMORP magic",
        )]);
    }
    let schema = cursor.read_u16("schema")?;
    if schema != MORPH_PACK_SCHEMA_VERSION && schema != MORPH_PACK_SKINNED_SCHEMA_VERSION {
        return Err(vec![error(
            "MORPH_PACK_UNSUPPORTED_SCHEMA",
            "schema",
            format!("expected schema {MORPH_PACK_SCHEMA_VERSION}"),
        )]);
    }
    let _flags = cursor.read_u16("flags")?;
    let manifest_len = usize::try_from(cursor.read_u32("manifestLength")?).map_err(|_| {
        vec![error(
            "MORPH_PACK_COUNT_OVERFLOW",
            "manifestLength",
            "manifest length exceeds the supported range",
        )]
    })?;
    if manifest_len > MAX_MORPH_PACK_MANIFEST_BYTES {
        return Err(vec![error(
            "MORPH_PACK_MANIFEST_TOO_LARGE",
            "manifestLength",
            format!("manifest exceeds {MAX_MORPH_PACK_MANIFEST_BYTES} bytes"),
        )]);
    }
    let manifest_bytes = cursor.read_bytes(manifest_len, "manifest")?;
    let manifest: MorphPackManifest =
        serde_json::from_slice(manifest_bytes).map_err(|parse_error| {
            vec![error(
                "MORPH_PACK_INVALID_MANIFEST",
                "manifest",
                parse_error.to_string(),
            )]
        })?;
    let asset_diagnostics = manifest.asset.validate();
    if !asset_diagnostics.is_empty() {
        return Err(asset_diagnostics);
    }
    validate_attachment(&manifest.attachment)?;
    let mode = match (schema, manifest.attachment.mode.as_str()) {
        (1, "rigid") => MorphPackAttachmentMode::Rigid,
        (MORPH_PACK_SKINNED_SCHEMA_VERSION, "skinned") => MorphPackAttachmentMode::Skinned,
        (1, _) => {
            return Err(vec![error(
                "MORPH_PACK_UNSUPPORTED_ATTACHMENT",
                "attachment.mode",
                "schema 1 only supports rigid attachments",
            )]);
        }
        (MORPH_PACK_SKINNED_SCHEMA_VERSION, _) => {
            return Err(vec![error(
                "MORPH_PACK_UNSUPPORTED_ATTACHMENT",
                "attachment.mode",
                "schema 2 requires a skinned attachment",
            )]);
        }
        _ => unreachable!(),
    };

    let skinned = mode == MorphPackAttachmentMode::Skinned;
    let mut lods = Vec::with_capacity(3);
    for level in ["near", "mid", "far"] {
        lods.push(read_lod(&mut cursor, level, skinned)?);
    }
    if cursor.remaining() != 0 {
        return Err(vec![error(
            "MORPH_PACK_TRAILING_BYTES",
            "$",
            format!("pack has {} trailing bytes", cursor.remaining()),
        )]);
    }
    let lods: [MorphPackLod; 3] = lods.try_into().map_err(|_| {
        vec![error(
            "MORPH_PACK_INVALID_LOD_COUNT",
            "lods",
            "pack must contain near, mid, and far LODs",
        )]
    })?;
    validate_attached_bounds(&manifest.attachment, &lods)?;
    let declared = [
        manifest.asset.lod.near,
        manifest.asset.lod.mid,
        manifest.asset.lod.far,
    ];
    for (index, (level, lod)) in ["near", "mid", "far"].into_iter().zip(&lods).enumerate() {
        if lod.triangle_count > declared[index] {
            return Err(vec![error(
                "MORPH_PACK_TRIANGLE_BUDGET_EXCEEDED",
                &format!("asset.lod.{level}"),
                format!(
                    "asset budget is {}, pack payload contains {}",
                    declared[index], lod.triangle_count
                ),
            )]);
        }
    }
    Ok(MorphPack {
        asset: manifest.asset,
        attachment: MorphPackAttachment {
            mode,
            joint: manifest.attachment.joint,
            translation: manifest.attachment.translation,
            rotation: manifest.attachment.rotation,
            scale: manifest.attachment.scale,
        },
        lods,
    })
}

fn validate_attached_bounds(
    attachment: &MorphPackManifestAttachment,
    lods: &[MorphPackLod; 3],
) -> Result<(), Vec<MorphDiagnostic>> {
    let [qx, qy, qz, qw] = attachment.rotation;
    for (level, lod) in ["near", "mid", "far"].into_iter().zip(lods) {
        for (index, vertex) in lod.vertices.iter().enumerate() {
            let scaled = [
                vertex[0] * attachment.scale[0],
                vertex[1] * attachment.scale[1],
                vertex[2] * attachment.scale[2],
            ];
            let cross = |a: [f32; 3], b: [f32; 3]| {
                [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ]
            };
            let q = [qx, qy, qz];
            let twice_cross = cross(q, scaled).map(|value| value * 2.0);
            let second_cross = cross(q, twice_cross);
            let attached: [f32; 3] = std::array::from_fn(|axis| {
                scaled[axis]
                    + twice_cross[axis] * qw
                    + second_cross[axis]
                    + attachment.translation[axis]
            });
            if attached
                .iter()
                .any(|value| !value.is_finite() || value.abs() > MAX_ATTACHED_VERTEX_ABS)
            {
                return Err(vec![error(
                    "MORPH_PACK_ATTACHED_BOUNDS",
                    &format!("lods.{level}.vertices[{index}]"),
                    format!(
                        "attached vertex must remain finite and within +/-{MAX_ATTACHED_VERTEX_ABS} units"
                    ),
                )]);
            }
        }
    }
    Ok(())
}

fn read_lod(
    cursor: &mut Cursor<'_>,
    level: &str,
    skinned: bool,
) -> Result<MorphPackLod, Vec<MorphDiagnostic>> {
    let triangle_count = cursor.read_u32(&format!("lods.{level}.triangleCount"))?;
    let vertex_count = bounded_count(
        cursor.read_u32(&format!("lods.{level}.vertexCount"))?,
        MAX_MORPH_PACK_VERTICES,
        &format!("lods.{level}.vertexCount"),
    )?;
    let index_count = bounded_count(
        cursor.read_u32(&format!("lods.{level}.indexCount"))?,
        MAX_MORPH_PACK_INDICES,
        &format!("lods.{level}.indexCount"),
    )?;
    let has_color = cursor.read_u8(&format!("lods.{level}.hasColor"))?;
    let base_color = match has_color {
        0 => None,
        1 => Some(cursor.read_color(&format!("lods.{level}.baseColor"))?),
        _ => {
            return Err(vec![error(
                "MORPH_PACK_INVALID_COLOR_FLAG",
                &format!("lods.{level}.hasColor"),
                "color flag must be 0 or 1",
            )]);
        }
    };
    let mut vertices = Vec::with_capacity(vertex_count);
    for vertex_index in 0..vertex_count {
        let vertex = cursor.read_vertex(&format!("lods.{level}.vertices[{vertex_index}]"))?;
        vertices.push(vertex);
    }
    let mut skinning = skinned.then(|| Vec::with_capacity(vertex_count));
    if let Some(skinning) = &mut skinning {
        for vertex_index in 0..vertex_count {
            skinning.push(cursor.read_skin(&format!("lods.{level}.skinning[{vertex_index}]"))?);
        }
    }
    let mut indices = Vec::with_capacity(index_count);
    for index in 0..index_count {
        let value = cursor.read_u32(&format!("lods.{level}.indices[{index}]"))?;
        let Ok(value_index) = usize::try_from(value) else {
            return Err(vec![error(
                "MORPH_PACK_INVALID_INDEX",
                &format!("lods.{level}.indices[{index}]"),
                "index exceeds the supported vertex range",
            )]);
        };
        if value_index >= vertex_count {
            return Err(vec![error(
                "MORPH_PACK_INVALID_INDEX",
                &format!("lods.{level}.indices[{index}]"),
                format!("index {value} references vertex count {vertex_count}"),
            )]);
        }
        indices.push(value);
    }
    let triangle_count_usize = usize::try_from(triangle_count).map_err(|_| {
        vec![error(
            "MORPH_PACK_COUNT_OVERFLOW",
            &format!("lods.{level}.triangleCount"),
            "triangle count exceeds the supported range",
        )]
    })?;
    if index_count % 3 != 0 || index_count / 3 != triangle_count_usize {
        return Err(vec![error(
            "MORPH_PACK_TRIANGLE_BUDGET_EXCEEDED",
            &format!("lods.{level}"),
            format!(
                "triangle count {triangle_count} does not match {index_count} triangle-list indices"
            ),
        )]);
    }
    Ok(MorphPackLod {
        triangle_count,
        vertices,
        indices,
        base_color,
        skinning,
    })
}

fn bounded_count(count: u32, limit: usize, path: &str) -> Result<usize, Vec<MorphDiagnostic>> {
    let count = usize::try_from(count).map_err(|_| {
        vec![error(
            "MORPH_PACK_COUNT_OVERFLOW",
            path,
            "count exceeds the supported range",
        )]
    })?;
    if count > limit {
        return Err(vec![error(
            "MORPH_PACK_RESOURCE_LIMIT",
            path,
            format!("count exceeds limit {limit}"),
        )]);
    }
    Ok(count)
}

fn validate_attachment(
    attachment: &MorphPackManifestAttachment,
) -> Result<(), Vec<MorphDiagnostic>> {
    let mut diagnostics = Vec::new();
    if attachment.joint.is_empty()
        || attachment.joint.len() > 96
        || !attachment.joint.is_ascii()
        || !attachment.joint.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        diagnostics.push(error(
            "MORPH_PACK_INVALID_ATTACHMENT_JOINT",
            "attachment.joint",
            "joint must be a lower-case semantic name",
        ));
    }
    if !attachment
        .translation
        .iter()
        .all(|value| value.is_finite() && value.abs() <= 10.0)
    {
        diagnostics.push(error(
            "MORPH_PACK_INVALID_ATTACHMENT_TRANSLATION",
            "attachment.translation",
            "translation values must be finite and within +/-10 units",
        ));
    }
    if !attachment.rotation.iter().all(|value| value.is_finite()) {
        diagnostics.push(error(
            "MORPH_PACK_INVALID_ATTACHMENT_ROTATION",
            "attachment.rotation",
            "rotation values must be finite",
        ));
    } else {
        let length = attachment
            .rotation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if !(0.99..=1.01).contains(&length) {
            diagnostics.push(error(
                "MORPH_PACK_INVALID_ATTACHMENT_ROTATION",
                "attachment.rotation",
                "rotation quaternion must be normalized",
            ));
        }
    }
    if !attachment
        .scale
        .iter()
        .all(|value| value.is_finite() && (0.01..=100.0).contains(value))
    {
        diagnostics.push(error(
            "MORPH_PACK_INVALID_ATTACHMENT_SCALE",
            "attachment.scale",
            "scale values must be finite and within 0.01..=100",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn read_bytes(&mut self, length: usize, path: &str) -> Result<&'a [u8], Vec<MorphDiagnostic>> {
        let end = self.offset.checked_add(length).ok_or_else(|| {
            vec![error(
                "MORPH_PACK_TRUNCATED",
                path,
                "field length overflows the pack",
            )]
        })?;
        if end > self.bytes.len() {
            return Err(vec![error(
                "MORPH_PACK_TRUNCATED",
                path,
                "pack ends before this field is complete",
            )]);
        }
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn read_u8(&mut self, path: &str) -> Result<u8, Vec<MorphDiagnostic>> {
        Ok(*self.read_bytes(1, path)?.first().unwrap_or(&0))
    }

    fn read_u16(&mut self, path: &str) -> Result<u16, Vec<MorphDiagnostic>> {
        let bytes = self.read_bytes(2, path)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_skin(&mut self, path: &str) -> Result<MorphPackVertexSkin, Vec<MorphDiagnostic>> {
        let joints = [
            self.read_u16(path)?,
            self.read_u16(path)?,
            self.read_u16(path)?,
            self.read_u16(path)?,
        ];
        let weights = [
            self.read_f32(path)?,
            self.read_f32(path)?,
            self.read_f32(path)?,
            self.read_f32(path)?,
        ];
        if joints.iter().any(|joint| *joint >= 15)
            || weights
                .iter()
                .any(|weight| !weight.is_finite() || !(0.0..=1.0).contains(weight))
            || (weights.iter().sum::<f32>() - 1.0).abs() > 0.01
        {
            return Err(vec![error(
                "MORPH_PACK_INVALID_SKIN",
                path,
                "skin joints must reference the 15-joint rig and weights must be finite, normalized, and within 0..=1",
            )]);
        }
        Ok(MorphPackVertexSkin { joints, weights })
    }

    fn read_u32(&mut self, path: &str) -> Result<u32, Vec<MorphDiagnostic>> {
        let bytes = self.read_bytes(4, path)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_vertex(&mut self, path: &str) -> Result<[f32; 3], Vec<MorphDiagnostic>> {
        let values = [
            self.read_f32(path)?,
            self.read_f32(path)?,
            self.read_f32(path)?,
        ];
        if values.iter().all(|value| value.is_finite()) {
            Ok(values)
        } else {
            Err(vec![error(
                "MORPH_PACK_NONFINITE_VERTEX",
                path,
                "vertex values must be finite",
            )])
        }
    }

    fn read_color(&mut self, path: &str) -> Result<[f32; 4], Vec<MorphDiagnostic>> {
        let color = [
            self.read_f32(path)?,
            self.read_f32(path)?,
            self.read_f32(path)?,
            self.read_f32(path)?,
        ];
        if color
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            Ok(color)
        } else {
            Err(vec![error(
                "MORPH_PACK_INVALID_COLOR",
                path,
                "base color values must be finite and within 0..=1",
            )])
        }
    }

    fn read_f32(&mut self, path: &str) -> Result<f32, Vec<MorphDiagnostic>> {
        let bytes = self.read_bytes(4, path)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

fn error(code: &str, path: &str, message: impl Into<String>) -> MorphDiagnostic {
    MorphDiagnostic {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Vec<u8> {
        let manifest = serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "asset": {
                "id": "cuba:headwear/pack-test.v1",
                "kind": "headwear",
                "displayName": "Pack Test",
                "rigProfile": "cuba:rig/biped15.v1",
                "fitProfiles": ["cuba:fit/person-standard.v1"],
                "supportedBases": ["cuba:base/person.v1"],
                "occupiedSlots": ["headwear"],
                "coverage": ["head"],
                "conflicts": [],
                "materials": ["default"],
                "lod": {"near": 1, "mid": 1, "far": 1},
                "requiredCapabilities": ["mesh.rigid.v1"],
                "provenance": {"source": "test", "license": "test"}
            },
            "attachment": {
                "mode": "rigid",
                "joint": "head",
                "translation": [0.0, 0.0, 0.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0]
            }
        }))
        .unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MORPH_PACK_MAGIC);
        bytes.extend_from_slice(&MORPH_PACK_SCHEMA_VERSION.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&manifest);
        for _ in 0..3 {
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&3u32.to_le_bytes());
            bytes.extend_from_slice(&3u32.to_le_bytes());
            bytes.push(1);
            for value in [0.2f32, 0.4, 0.8, 1.0] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            for vertex in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
                for value in vertex {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            for index in [0u32, 1, 2] {
                bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
        bytes
    }

    #[test]
    fn decodes_a_bounded_pack() {
        let pack = decode_morph_pack(&fixture()).expect("pack should decode");
        assert_eq!(pack.asset.id.as_str(), "cuba:headwear/pack-test.v1");
        assert_eq!(pack.attachment.joint, "head");
        assert_eq!(pack.lods[1].vertices.len(), 3);
        assert_eq!(pack.lods[2].base_color, Some([0.2, 0.4, 0.8, 1.0]));
    }

    #[test]
    fn rejects_trailing_pack_bytes() {
        let mut bytes = fixture();
        bytes.push(0);
        let diagnostics = decode_morph_pack(&bytes).unwrap_err();
        assert_eq!(diagnostics[0].code, "MORPH_PACK_TRAILING_BYTES");
    }

    #[test]
    fn rejects_geometry_that_becomes_unbounded_after_attachment() {
        let attachment = MorphPackManifestAttachment {
            mode: "rigid".to_owned(),
            joint: "head".to_owned(),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [100.0; 3],
        };
        let lod = MorphPackLod {
            triangle_count: 1,
            vertices: vec![[2.0, 0.0, 0.0]],
            indices: vec![0, 0, 0],
            base_color: None,
            skinning: None,
        };
        let diagnostics =
            validate_attached_bounds(&attachment, &[lod.clone(), lod.clone(), lod]).unwrap_err();
        assert_eq!(diagnostics[0].code, "MORPH_PACK_ATTACHED_BOUNDS");
    }
}
