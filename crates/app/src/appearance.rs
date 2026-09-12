use cubacadabra_morphs::{
    CapabilitySet, MORPH_CATALOG_SCHEMA_VERSION, MorphAssetDefinition, MorphAssetKind,
    MorphCatalog, MorphLoadout, MorphPreset, resolve_loadout,
};
use serde::{Deserialize, Serialize};

use crate::EffectId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearanceFeedback {
    pub kind: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MorphAssetSnapshot {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub thumbnail: Option<String>,
    pub artifact_url: Option<String>,
    pub supported_bases: Vec<String>,
    pub occupied_slots: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MorphPresetSnapshot {
    pub id: String,
    pub display_name: String,
    pub base: String,
    pub parts: Vec<String>,
    pub face: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AppearanceSnapshot {
    pub release: Option<String>,
    pub assets: Vec<MorphAssetSnapshot>,
    pub presets: Vec<MorphPresetSnapshot>,
    pub selected_base: Option<String>,
    pub selected_parts: Vec<String>,
    pub selected_face: Option<String>,
    pub draft_base: Option<String>,
    pub draft_parts: Vec<String>,
    pub draft_face: Option<String>,
    pub draft_preset_id: Option<String>,
    pub selected_loadout_json: Option<String>,
    pub draft_loadout_json: Option<String>,
    pub draft_can_save: bool,
    pub is_loading: bool,
    pub is_saving: bool,
    pub feedback: Option<AppearanceFeedback>,
}

#[derive(Debug, Default)]
pub(crate) struct AppearanceState {
    release: Option<String>,
    catalog: Vec<MorphAssetDefinition>,
    assets: Vec<MorphAssetSnapshot>,
    presets: Vec<MorphPreset>,
    saved: Option<MorphLoadout>,
    draft: Option<MorphLoadout>,
    editing: bool,
    revision: u32,
    pending: Option<PendingAppearanceRequest>,
    feedback: Option<AppearanceFeedback>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingAppearanceRequest {
    Catalog(EffectId),
    Load(EffectId),
    Save(EffectId),
}

#[derive(Deserialize)]
struct CatalogResponse {
    #[serde(default)]
    assets: Vec<RemoteAsset>,
    #[serde(default)]
    presets: Vec<MorphPreset>,
    release: Option<String>,
}

#[derive(Deserialize)]
struct RemoteAsset {
    id: String,
    kind: String,
    #[serde(default)]
    name: String,
    thumbnail: Option<String>,
    #[serde(default)]
    artifact: Option<RemoteArtifact>,
    #[serde(default)]
    definition: Option<MorphAssetDefinition>,
}

#[derive(Deserialize)]
struct RemoteArtifact {
    url: Option<String>,
}

#[derive(Deserialize)]
struct AppearanceResponse {
    appearance: serde_json::Value,
    #[serde(default)]
    revision: u32,
}

impl AppearanceState {
    pub(crate) fn replace(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn request_catalog(&mut self, effect_id: EffectId) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(PendingAppearanceRequest::Catalog(effect_id));
        self.feedback = None;
        true
    }

    pub(crate) fn request_load(&mut self, effect_id: EffectId) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(PendingAppearanceRequest::Load(effect_id));
        self.feedback = None;
        true
    }

    pub(crate) fn request_save(&mut self, effect_id: EffectId) -> Option<(MorphLoadout, u32)> {
        let draft = self.draft.clone()?;
        if self.pending.is_some() || self.saved.as_ref() == Some(&draft) {
            return None;
        }
        self.pending = Some(PendingAppearanceRequest::Save(effect_id));
        self.feedback = None;
        Some((draft, self.revision))
    }

    pub(crate) fn is_pending(&self, effect_id: EffectId) -> bool {
        matches!(self.pending, Some(PendingAppearanceRequest::Catalog(id) | PendingAppearanceRequest::Load(id) | PendingAppearanceRequest::Save(id)) if id == effect_id)
    }

    pub(crate) fn pending_kind(&self, effect_id: EffectId) -> Option<PendingAppearanceRequest> {
        match self.pending {
            Some(request @ PendingAppearanceRequest::Catalog(id))
            | Some(request @ PendingAppearanceRequest::Load(id))
            | Some(request @ PendingAppearanceRequest::Save(id))
                if id == effect_id =>
            {
                Some(request)
            }
            _ => None,
        }
    }

    pub(crate) fn catalog_loaded(&mut self, effect_id: EffectId, response: &str) -> Result<(), ()> {
        if self.pending != Some(PendingAppearanceRequest::Catalog(effect_id)) {
            return Err(());
        }
        let parsed: CatalogResponse = serde_json::from_str(response).map_err(|_| ())?;
        let mut definitions = Vec::new();
        let mut assets = Vec::new();
        for remote in parsed.assets {
            let Some(definition) = remote.definition else {
                continue;
            };
            let kind = serde_json::from_str::<MorphAssetKind>(&format!("\"{}\"", remote.kind))
                .map_err(|_| ())?;
            if definition.kind != kind {
                return Err(());
            }
            assets.push(MorphAssetSnapshot {
                id: remote.id,
                kind: remote.kind,
                display_name: if remote.name.is_empty() {
                    definition.display_name.clone()
                } else {
                    remote.name
                },
                thumbnail: remote.thumbnail,
                artifact_url: remote.artifact.and_then(|artifact| artifact.url),
                supported_bases: definition
                    .supported_bases
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                occupied_slots: definition.occupied_slots.clone(),
            });
            definitions.push(definition);
        }
        self.release = parsed.release;
        self.catalog = definitions;
        self.assets = assets;
        self.presets = parsed.presets;
        self.assets.retain(|asset| {
            asset.kind == "face"
                || asset
                    .artifact_url
                    .as_deref()
                    .is_some_and(|url| !url.is_empty())
        });
        let authored_presets = self
            .presets
            .iter()
            .filter(|preset| self.loadout_is_renderable(&preset.loadout()))
            .cloned()
            .collect();
        self.presets = authored_presets;
        self.pending = None;
        self.feedback = None;
        self.repair_unrenderable_draft();
        Ok(())
    }

    pub(crate) fn appearance_loaded(
        &mut self,
        effect_id: EffectId,
        status: u16,
        response: &str,
    ) -> Result<(), ()> {
        if self.pending != Some(PendingAppearanceRequest::Load(effect_id)) {
            return Err(());
        }
        if !(200..300).contains(&status) {
            return Err(());
        }
        let parsed: AppearanceResponse = serde_json::from_str(response).map_err(|_| ())?;
        let loadout: MorphLoadout = serde_json::from_value(parsed.appearance).map_err(|_| ())?;
        self.revision = parsed.revision;
        self.saved = Some(loadout.clone());
        self.draft = Some(loadout);
        self.pending = None;
        self.feedback = None;
        self.repair_unrenderable_draft();
        Ok(())
    }

    pub(crate) fn appearance_saved(
        &mut self,
        effect_id: EffectId,
        status: u16,
        response: &str,
    ) -> Result<(), ()> {
        if self.pending != Some(PendingAppearanceRequest::Save(effect_id))
            || !(200..300).contains(&status)
        {
            return Err(());
        }
        let parsed: AppearanceResponse = serde_json::from_str(response).map_err(|_| ())?;
        let loadout: MorphLoadout = serde_json::from_value(parsed.appearance).map_err(|_| ())?;
        self.revision = parsed.revision;
        self.saved = Some(loadout.clone());
        self.draft = Some(loadout);
        self.pending = None;
        self.feedback = Some(AppearanceFeedback {
            kind: "success".into(),
            code: "saved".into(),
            message: "Morph saved.".into(),
        });
        Ok(())
    }

    pub(crate) fn failed(&mut self, effect_id: EffectId) {
        if !self.is_pending(effect_id) {
            return;
        }
        self.pending = None;
        self.feedback = Some(AppearanceFeedback {
            kind: "error".into(),
            code: "unavailable".into(),
            message: "We couldn’t load your morph. Please try again.".into(),
        });
    }

    pub(crate) fn begin_edit(&mut self) {
        self.editing = true;
        if self.draft.is_none() {
            self.draft = self.saved.clone();
        }
        self.feedback = None;
        self.repair_unrenderable_draft();
    }

    fn morph_catalog(&self) -> MorphCatalog {
        MorphCatalog {
            schema_version: MORPH_CATALOG_SCHEMA_VERSION,
            content_version: self.release.clone().unwrap_or_else(|| "local".into()),
            assets: self.catalog.clone(),
            presets: self.presets.clone(),
        }
    }

    fn repair_unrenderable_draft(&mut self) {
        if !self.editing {
            return;
        }
        if let Some(draft) = &self.draft
            && self.loadout_resolves(draft)
        {
            if let Some(id) = self.missing_artifact_id(draft) {
                self.feedback = Some(AppearanceFeedback {
                    kind: "error".into(),
                    code: "missing_morph_artifact".into(),
                    message: format!("Morph asset {id} has no schema-5 artifact."),
                });
            }
            return;
        }
        if let Some(preset) = self.presets.first() {
            self.draft = Some(preset.loadout());
        }
    }

    fn loadout_resolves(&self, loadout: &MorphLoadout) -> bool {
        let catalog = self.morph_catalog();
        let capabilities = CapabilitySet::new(
            catalog
                .assets
                .iter()
                .flat_map(|asset| asset.required_capabilities.iter().cloned()),
        );
        resolve_loadout(&catalog, loadout, &capabilities).is_ok()
    }

    fn missing_artifact_id<'a>(&self, loadout: &'a MorphLoadout) -> Option<&'a str> {
        let catalog = self.morph_catalog();
        std::iter::once(&loadout.base)
            .chain(loadout.parts.iter())
            .chain(loadout.face.iter())
            .find_map(|id| {
                let needs_artifact = catalog
                    .asset(id)
                    .is_some_and(|asset| asset.kind != MorphAssetKind::Face);
                (needs_artifact
                    && !self.assets.iter().any(|row| {
                        row.id == id.as_str()
                            && row
                                .artifact_url
                                .as_deref()
                                .is_some_and(|url| !url.is_empty())
                    }))
                .then_some(id.as_str())
            })
    }

    fn loadout_is_renderable(&self, loadout: &MorphLoadout) -> bool {
        self.loadout_resolves(loadout) && self.missing_artifact_id(loadout).is_none()
    }

    pub(crate) fn select_preset(&mut self, preset_id: &str) {
        let Some(preset) = self
            .presets
            .iter()
            .find(|preset| preset.id.as_str() == preset_id)
        else {
            return;
        };
        self.draft = Some(preset.loadout());
        self.feedback = None;
    }

    pub(crate) fn set_part(&mut self, asset_id: &str) {
        let Some(asset) = self
            .catalog
            .iter()
            .find(|asset| asset.id.as_str() == asset_id)
        else {
            return;
        };
        if asset.kind != MorphAssetKind::Face
            && !self.assets.iter().any(|row| {
                row.id == asset_id
                    && row
                        .artifact_url
                        .as_deref()
                        .is_some_and(|url| !url.is_empty())
            })
        {
            return;
        }
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        match asset.kind {
            MorphAssetKind::Base => draft.base = asset.id.clone(),
            MorphAssetKind::Face => draft.face = Some(asset.id.clone()),
            _ => {
                draft.parts.retain(|id| {
                    self.catalog
                        .iter()
                        .find(|candidate| candidate.id == *id)
                        .map(|candidate| candidate.kind != asset.kind)
                        .unwrap_or(true)
                });
                draft.parts.push(asset.id.clone());
            }
        }
        draft.canonicalize();
        self.feedback = None;
    }

    pub(crate) fn clear_part(&mut self, asset_id: &str) {
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        draft.parts.retain(|id| id.as_str() != asset_id);
        if draft
            .face
            .as_ref()
            .is_some_and(|id| id.as_str() == asset_id)
        {
            draft.face = None;
        }
    }

    pub(crate) fn snapshot(&self) -> AppearanceSnapshot {
        let selections = |loadout: Option<&MorphLoadout>| {
            (
                loadout.map(|v| v.base.to_string()),
                loadout
                    .map(|v| v.parts.iter().map(ToString::to_string).collect())
                    .unwrap_or_default(),
                loadout.and_then(|v| v.face.as_ref().map(ToString::to_string)),
            )
        };
        let (selected_base, selected_parts, selected_face) = selections(self.saved.as_ref());
        let (draft_base, draft_parts, draft_face) = selections(self.draft.as_ref());
        let loadout_json = |loadout: Option<&MorphLoadout>| {
            loadout
                .filter(|loadout| self.loadout_is_renderable(loadout))
                .and_then(|loadout| serde_json::to_string(loadout).ok())
        };
        let draft_preset_id = self.draft.as_ref().and_then(|draft| {
            self.presets
                .iter()
                .find(|preset| loadout_matches_preset(draft, preset))
                .map(|preset| preset.id.to_string())
        });
        AppearanceSnapshot {
            release: self.release.clone(),
            assets: self.assets.clone(),
            presets: self.presets.iter().map(preset_snapshot).collect(),
            selected_base,
            selected_parts,
            selected_face,
            draft_base,
            draft_parts,
            draft_face,
            draft_preset_id,
            selected_loadout_json: loadout_json(self.saved.as_ref()),
            draft_loadout_json: loadout_json(self.draft.as_ref()),
            draft_can_save: self.draft.as_ref().is_some_and(|draft| {
                self.loadout_is_renderable(draft) && self.saved.as_ref() != Some(draft)
            }),
            is_loading: matches!(
                self.pending,
                Some(PendingAppearanceRequest::Catalog(_) | PendingAppearanceRequest::Load(_))
            ),
            is_saving: matches!(self.pending, Some(PendingAppearanceRequest::Save(_))),
            feedback: self.feedback.clone(),
        }
    }
}

fn loadout_matches_preset(loadout: &MorphLoadout, preset: &MorphPreset) -> bool {
    let mut left = loadout.clone();
    let mut right = preset.loadout();
    left.revision = 0;
    right.revision = 0;
    left.canonicalize();
    right.canonicalize();
    left == right
}

fn preset_snapshot(preset: &MorphPreset) -> MorphPresetSnapshot {
    MorphPresetSnapshot {
        id: preset.id.to_string(),
        display_name: preset.display_name.clone(),
        base: preset.base.to_string(),
        parts: preset.parts.iter().map(ToString::to_string).collect(),
        face: preset.face.as_ref().map(ToString::to_string),
        thumbnail: preset.thumbnail.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubacadabra_morphs::parse_catalog;

    fn asset_snapshots(assets: &[MorphAssetDefinition]) -> Vec<MorphAssetSnapshot> {
        assets
            .iter()
            .map(|asset| MorphAssetSnapshot {
                id: asset.id.to_string(),
                kind: serde_json::to_value(asset.kind)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
                display_name: asset.display_name.clone(),
                thumbnail: None,
                artifact_url: (asset.kind != MorphAssetKind::Face)
                    .then(|| format!("/morphs/packs/{}.morphpack", asset.id)),
                supported_bases: asset
                    .supported_bases
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                occupied_slots: asset.occupied_slots.clone(),
            })
            .collect()
    }

    fn current_catalog() -> (
        Vec<MorphAssetDefinition>,
        Vec<MorphAssetSnapshot>,
        MorphPreset,
    ) {
        let catalog = parse_catalog(include_str!(
            "../../../assets/characters/morph_catalog.json"
        ))
        .unwrap();
        let assets: Vec<MorphAssetDefinition> = catalog
            .assets
            .into_iter()
            .filter(|asset| asset.id.as_str() != "cuba:everyday-hoodie.v1")
            .collect();
        let preset = serde_json::from_value(serde_json::json!({
            "id": "cuba:preset/current.v1",
            "displayName": "Current starter",
            "base": "cuba:base/person-02.v1",
            "parts": ["cuba:hair/shag.v1"],
            "face": "cuba:face/neutral.v1",
            "parameters": {}
        }))
        .unwrap();
        let snapshots = asset_snapshots(&assets);
        (assets, snapshots, preset)
    }

    fn retired_loadout() -> MorphLoadout {
        serde_json::from_value(serde_json::json!({
            "version": 2,
            "base": "cuba:base/person.v1",
            "parts": [
                "cuba:hair/swept.v1",
                "cuba:everyday-hoodie.v1"
            ],
            "parameters": {},
            "revision": 0
        }))
        .unwrap()
    }

    #[test]
    fn beginning_an_edit_replaces_an_unrenderable_loadout_with_the_catalog_starter() {
        let retired = retired_loadout();
        let (catalog, assets, preset) = current_catalog();
        let mut state = AppearanceState {
            catalog,
            assets,
            presets: vec![preset.clone()],
            saved: Some(retired.clone()),
            draft: Some(retired.clone()),
            ..Default::default()
        };

        state.begin_edit();

        assert_eq!(state.saved, Some(retired));
        assert_eq!(state.draft, Some(preset.loadout()));
        assert!(state.snapshot().draft_can_save);
        assert_eq!(
            state.snapshot().draft_preset_id.as_deref(),
            Some("cuba:preset/current.v1")
        );
    }

    #[test]
    fn a_late_appearance_response_still_repairs_an_unrenderable_loadout() {
        let retired = retired_loadout();
        let (catalog, assets, preset) = current_catalog();
        let mut state = AppearanceState {
            catalog,
            assets,
            presets: vec![preset.clone()],
            pending: Some(PendingAppearanceRequest::Load(EffectId(7))),
            ..Default::default()
        };
        state.begin_edit();

        state
            .appearance_loaded(
                EffectId(7),
                200,
                &serde_json::json!({
                    "appearance": retired,
                    "revision": 3
                })
                .to_string(),
            )
            .unwrap();

        assert_eq!(state.saved, Some(retired_loadout()));
        assert_eq!(state.draft, Some(preset.loadout()));
    }

    #[test]
    fn beginning_an_edit_preserves_a_renderable_custom_loadout() {
        let (catalog, assets, preset) = current_catalog();
        let custom: MorphLoadout = serde_json::from_value(serde_json::json!({
            "version": 2,
            "base": "cuba:base/person.v1",
            "parts": [],
            "face": "cuba:face/happy.v1",
            "parameters": {},
            "revision": 0
        }))
        .unwrap();
        let mut state = AppearanceState {
            catalog,
            assets,
            presets: vec![preset],
            saved: Some(custom.clone()),
            draft: Some(custom.clone()),
            ..Default::default()
        };

        state.begin_edit();

        assert_eq!(state.saved, Some(custom.clone()));
        assert_eq!(state.draft, Some(custom));
        assert!(!state.snapshot().draft_can_save);
    }

    #[test]
    fn a_selected_asset_without_a_pack_is_reported_instead_of_replaced() {
        let (catalog, mut assets, preset) = current_catalog();
        let custom: MorphLoadout = serde_json::from_value(serde_json::json!({
            "version": 2,
            "base": "cuba:base/person.v1",
            "parts": ["cuba:hair/shag.v1"],
            "parameters": {},
            "revision": 0
        }))
        .unwrap();
        assets.retain(|asset| asset.id != "cuba:hair/shag.v1");
        let mut state = AppearanceState {
            catalog,
            assets,
            presets: vec![preset],
            saved: Some(custom.clone()),
            draft: Some(custom.clone()),
            ..Default::default()
        };

        state.begin_edit();

        assert_eq!(state.draft, Some(custom));
        assert_eq!(state.snapshot().draft_loadout_json, None);
        assert_eq!(
            state.snapshot().feedback.unwrap().code,
            "missing_morph_artifact"
        );
    }

    #[test]
    fn snapshot_exposes_the_native_loadout_and_identifies_one_exact_preset() {
        let catalog = parse_catalog(include_str!(
            "../../../assets/characters/morph_catalog.json"
        ))
        .unwrap();
        let preset = catalog.presets[0].clone();
        let mut state = AppearanceState {
            release: Some(catalog.content_version.clone()),
            assets: asset_snapshots(&catalog.assets),
            catalog: catalog.assets,
            presets: catalog.presets,
            draft: Some(preset.loadout()),
            ..Default::default()
        };

        let snapshot = state.snapshot();
        assert_eq!(
            snapshot.draft_preset_id.as_deref(),
            Some(preset.id.as_str())
        );
        let rendered: MorphLoadout =
            serde_json::from_str(snapshot.draft_loadout_json.as_deref().unwrap()).unwrap();
        assert_eq!(rendered.base, preset.base);

        state.set_part("cuba:face/curious.v1");
        assert_eq!(state.snapshot().draft_preset_id, None);
    }
}
