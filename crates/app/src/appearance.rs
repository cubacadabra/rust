use cubacadabra_morphs::{
    MorphAssetDefinition, MorphAssetId, MorphAssetKind, MorphLoadout, MorphPreset,
    migrate_v1_appearance,
};
use serde::{Deserialize, Serialize};

use crate::EffectId;

const PERSON_ONE_BASE: &str = "cuba:base/person.v1";
const PERSON_ONE_HAIR: &str = "cuba:hair/swept.v1";
const PERSON_ONE_TOP: &str = "cuba:everyday-hoodie.v1";

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
    definition: Option<MorphAssetDefinition>,
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
        self.pending = None;
        self.feedback = None;
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
        let loadout = if parsed
            .appearance
            .get("version")
            .and_then(serde_json::Value::as_u64)
            == Some(2)
        {
            serde_json::from_value(parsed.appearance).map_err(|_| ())?
        } else {
            let legacy = serde_json::from_value(parsed.appearance).map_err(|_| ())?;
            migrate_v1_appearance(&legacy).map_err(|_| ())?
        };
        self.revision = parsed.revision;
        self.saved = Some(loadout.clone());
        self.draft = Some(loadout);
        self.pending = None;
        self.feedback = None;
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
        if self.draft.is_none() {
            self.draft = Some(self.saved.clone().unwrap_or_else(person_one_loadout));
        }
        self.feedback = None;
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
            draft_can_save: self.draft.is_some() && self.saved != self.draft,
            is_loading: matches!(
                self.pending,
                Some(PendingAppearanceRequest::Catalog(_) | PendingAppearanceRequest::Load(_))
            ),
            is_saving: matches!(self.pending, Some(PendingAppearanceRequest::Save(_))),
            feedback: self.feedback.clone(),
        }
    }
}

pub(crate) fn person_one_loadout() -> MorphLoadout {
    MorphLoadout {
        version: 2,
        base: MorphAssetId::parse(PERSON_ONE_BASE).expect("valid default base"),
        parts: vec![
            MorphAssetId::parse(PERSON_ONE_HAIR).expect("valid default hair"),
            MorphAssetId::parse(PERSON_ONE_TOP).expect("valid default top"),
        ],
        face: None,
        parameters: Default::default(),
        revision: 0,
    }
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
