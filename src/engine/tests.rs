use super::{DEFAULT_ORBIT_DISTANCE, Engine, MAX_AGENTS, SNAPSHOT_STRIDE};
use crate::character::definition::EquipmentSlot;
use crate::types::{
    Agent, AgentPhase, CharacterEntityKind, CharacterSupport, Input, LaunchPadPhase,
};
use crate::ui::{UiInsets, UiViewport};
use crate::world::{
    HazardVolume, HealthSettings, LadderAxis, LadderVolume, PhysicsSettings, RespawnMode,
    RespawnSettings, SafeZone, block_bounds,
};

mod core {
    use super::*;
    include!("tests/core.rs");
}

mod players {
    use super::*;
    include!("tests/players.rs");
}

mod worlds {
    use super::*;
    include!("tests/worlds.rs");
}
