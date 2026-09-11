//! Portable data contracts for Cubacadabra morphs.
//!
//! This crate deliberately has no renderer, filesystem, windowing, or Blender
//! dependencies. Studio authoring tools and every runtime client can therefore
//! share the same identifiers, bounded loadout format, compatibility mapping,
//! and validation behavior.

mod capability;
mod catalog;
mod diagnostic;
mod id;
mod legacy;
mod loadout;
mod pack;
mod resolve;

pub use capability::{CapabilityId, CapabilitySet};
pub use catalog::{
    MAX_CATALOG_ASSETS, MAX_CATALOG_BYTES, MAX_CATALOG_PRESETS, MORPH_CATALOG_SCHEMA_VERSION,
    MorphAssetDefinition, MorphAssetKind, MorphCatalog, MorphLodBudget, MorphPreset,
    MorphProvenance, MorphSourceReference, parse_catalog,
};
pub use diagnostic::MorphDiagnostic;
pub use id::{AssetIdError, MorphAssetId};
pub use legacy::{LegacyAppearance, migrate_v1_appearance, project_v2_to_v1};
pub use loadout::{
    MAX_LOADOUT_BYTES, MAX_PARAMETERS, MAX_PARTS, MORPH_LOADOUT_VERSION, MorphLoadout,
    MorphParameterValue, parse_loadout,
};
pub use pack::{
    MAX_MORPH_PACK_BYTES, MAX_MORPH_PACK_SURFACES, MAX_MORPH_PACK_TEXTURES,
    MAX_MORPH_TEXTURE_DIMENSION, MORPH_PACK_MAGIC, MORPH_PACK_MULTI_SURFACE_SCHEMA_VERSION,
    MORPH_PACK_SCHEMA_VERSION, MORPH_PACK_SKINNED_SCHEMA_VERSION,
    MORPH_PACK_TEXTURED_SCHEMA_VERSION, MorphPack, MorphPackAttachment, MorphPackAttachmentMode,
    MorphPackLod, MorphPackSurface, MorphPackTexture, MorphPackVertexSkin, decode_morph_pack,
};
pub use resolve::{ResolvedMorphLoadout, resolve_loadout, resolve_preset};
