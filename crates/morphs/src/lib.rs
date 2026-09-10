//! Portable data contracts for Cubacadabra morphs.
//!
//! This crate deliberately has no renderer, filesystem, windowing, or Blender
//! dependencies. Studio authoring tools and every runtime client can therefore
//! share the same identifiers, bounded loadout format, compatibility mapping,
//! and validation behavior.

mod capability;
mod diagnostic;
mod id;
mod legacy;
mod loadout;

pub use capability::{CapabilityId, CapabilitySet};
pub use diagnostic::MorphDiagnostic;
pub use id::{AssetIdError, MorphAssetId};
pub use legacy::{LegacyAppearance, migrate_v1_appearance};
pub use loadout::{
    MAX_LOADOUT_BYTES, MAX_PARAMETERS, MAX_PARTS, MORPH_LOADOUT_VERSION, MorphLoadout,
    MorphParameterValue, parse_loadout,
};
