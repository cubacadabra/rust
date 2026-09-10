use serde::{Deserialize, Serialize};

use crate::EffectId;

pub const DEFAULT_CATALOG_PAGE_SIZE: u16 = 20;
const MAX_CATALOG_PAGE_SIZE: u16 = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub cube_id: String,
    pub version: String,
    pub display_name: String,
    pub package_path: String,
    pub asset_base_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogFeedbackKind {
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogFeedback {
    pub kind: CatalogFeedbackKind,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSnapshot {
    pub entries: Vec<CatalogEntry>,
    pub is_loading: bool,
    pub feedback: Option<CatalogFeedback>,
}

#[derive(Debug)]
pub(crate) struct PendingCatalogLoad {
    effect_id: EffectId,
}

#[derive(Debug, Default)]
pub(crate) struct CatalogState {
    entries: Vec<CatalogEntry>,
    pending_load: Option<PendingCatalogLoad>,
    feedback: Option<CatalogFeedback>,
}

impl CatalogState {
    pub(crate) fn replace(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn request_load(&mut self, effect_id: EffectId, page_size: u16) -> Option<u16> {
        if self.pending_load.is_some() {
            return None;
        }
        let page_size = page_size.clamp(1, MAX_CATALOG_PAGE_SIZE);
        self.feedback = None;
        self.pending_load = Some(PendingCatalogLoad { effect_id });
        Some(page_size)
    }

    pub(crate) fn is_pending(&self, effect_id: EffectId) -> bool {
        self.pending_load
            .as_ref()
            .is_some_and(|pending| pending.effect_id == effect_id)
    }

    pub(crate) fn loaded(&mut self, effect_id: EffectId, entries: Vec<CatalogEntry>) {
        if self.take_matching(effect_id) {
            self.entries = entries;
            self.feedback = None;
        }
    }

    pub(crate) fn failed(&mut self, effect_id: EffectId, code: &str, message: &str) {
        if self.take_matching(effect_id) {
            self.feedback = Some(CatalogFeedback {
                kind: CatalogFeedbackKind::Error,
                code: code.to_owned(),
                message: message.to_owned(),
            });
        }
    }

    pub(crate) fn snapshot(&self) -> CatalogSnapshot {
        CatalogSnapshot {
            entries: self.entries.clone(),
            is_loading: self.pending_load.is_some(),
            feedback: self.feedback.clone(),
        }
    }

    fn take_matching(&mut self, effect_id: EffectId) -> bool {
        if self
            .pending_load
            .as_ref()
            .is_none_or(|pending| pending.effect_id != effect_id)
        {
            return false;
        }
        self.pending_load = None;
        true
    }
}

#[derive(Deserialize)]
struct CatalogResponse {
    cubes: Vec<RawCatalogEntry>,
}

#[derive(Deserialize)]
struct RawCatalogEntry {
    #[allow(dead_code)]
    id: i64,
    #[serde(rename = "cubeId")]
    cube_id: String,
    version: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[allow(dead_code)]
    #[serde(rename = "fileCount")]
    file_count: i64,
    #[serde(rename = "packagePath")]
    package_path: String,
    #[serde(rename = "assetBaseURL")]
    asset_base_url: Option<String>,
}

pub(crate) fn response(
    status: u16,
    body: &str,
) -> Result<Vec<CatalogEntry>, (&'static str, &'static str)> {
    if !(200..300).contains(&status) {
        return Err((
            "unavailable",
            "We couldn’t load the cubes. Please try again.",
        ));
    }
    let response: CatalogResponse = serde_json::from_str(body).map_err(|_| {
        (
            "invalid_response",
            "We couldn’t load the cubes. Please try again.",
        )
    })?;
    let mut entries = Vec::with_capacity(response.cubes.len());
    for cube in response.cubes {
        if !is_valid_cube_id(&cube.cube_id)
            || cube.version.trim().is_empty()
            || cube.display_name.trim().is_empty()
            || !cube.package_path.starts_with("/cubes/")
            || !cube.package_path.ends_with('/')
            || entries
                .iter()
                .any(|entry: &CatalogEntry| entry.cube_id == cube.cube_id)
        {
            continue;
        }
        entries.push(CatalogEntry {
            cube_id: cube.cube_id,
            version: cube.version,
            display_name: cube.display_name,
            package_path: cube.package_path,
            asset_base_url: cube.asset_base_url,
        });
    }
    Ok(entries)
}

fn is_valid_cube_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes[0] != b'-'
        && bytes[bytes.len() - 1] != b'-'
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}
