use crate::math::Vec2;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Input {
    pub(crate) forward: f32,
    pub(crate) strafe: f32,
    pub(crate) sprint: bool,
    pub(crate) jump: bool,
    pub(crate) look_x: f32,
    pub(crate) look_y: f32,
    pub(crate) zoom_delta: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Player {
    pub(crate) position: [f32; 3],
    pub(crate) velocity: [f32; 3],
    pub(crate) grounded: bool,
    pub(crate) moving: bool,
    pub(crate) sprinting: bool,
    pub(crate) walk_cycle: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RemotePlayer {
    pub(crate) position: [f32; 3],
    pub(crate) yaw: f32,
    pub(crate) moving: bool,
    pub(crate) sprinting: bool,
    pub(crate) walk_cycle: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CharacterEntityKind {
    LocalPlayer,
    LocalNpc,
    RemotePlayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CharacterEntityKey {
    pub(crate) kind: CharacterEntityKind,
    pub(crate) slot: usize,
    /// Zero is the legacy slot generation until a host supplies a spawn
    /// generation. The motion source identifies that fallback explicitly.
    pub(crate) generation: u32,
}

impl Default for CharacterEntityKey {
    fn default() -> Self {
        Self {
            kind: CharacterEntityKind::LocalPlayer,
            slot: 0,
            generation: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CharacterMotionSource {
    Simulation,
    LegacyRemote,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CharacterMotionEvent {
    #[default]
    None,
    Takeoff,
    Landing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CharacterEmote {
    #[default]
    None,
    Wave,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum CharacterSupport {
    Grounded { height: f32 },
    Airborne,
    #[default]
    Unknown,
}

/// Typed presentation input for characters. This is deliberately separate
/// from the public eight-float snapshot, whose suffix has entity-specific
/// meanings and remains a compatibility ABI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CharacterMotionSample {
    pub(crate) key: CharacterEntityKey,
    pub(crate) sequence: u64,
    pub(crate) time: f32,
    pub(crate) position: [f32; 3],
    pub(crate) facing_yaw: f32,
    pub(crate) look_yaw: f32,
    pub(crate) planar_velocity: Option<[f32; 2]>,
    pub(crate) vertical_velocity: Option<f32>,
    pub(crate) support: CharacterSupport,
    pub(crate) stride_phase: f32,
    pub(crate) moving: bool,
    pub(crate) sprinting: bool,
    pub(crate) source: CharacterMotionSource,
    pub(crate) event: CharacterMotionEvent,
    pub(crate) emote: CharacterEmote,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BuildBlock {
    pub(crate) position: [f32; 3],
    pub(crate) size: [f32; 3],
    pub(crate) color: u32,
    pub(crate) rotation: u8,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 11.5],
            velocity: [0.0; 3],
            grounded: true,
            moving: false,
            sprinting: false,
            walk_cycle: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentPhase {
    Entering,
    Roaming,
    Assembling,
    Assembled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LaunchPadPhase {
    Idle,
    Countdown,
    Launched,
}

impl LaunchPadPhase {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Countdown => 1,
            Self::Launched => 2,
        }
    }
}

impl AgentPhase {
    pub(crate) fn code(self) -> f32 {
        match self {
            Self::Entering => 0.0,
            Self::Roaming => 1.0,
            Self::Assembling => 2.0,
            Self::Assembled => 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Agent {
    pub(crate) position: [f32; 3],
    pub(crate) target: Vec2,
    pub(crate) meeting_target: Vec2,
    pub(crate) meeting_index: usize,
    pub(crate) phase: AgentPhase,
    pub(crate) spawned_at: f32,
    pub(crate) next_decision_at: f32,
    pub(crate) gather_at: f32,
    pub(crate) next_jump_at: f32,
    pub(crate) speed: f32,
    pub(crate) walk_cycle: f32,
    pub(crate) vertical_velocity: f32,
    pub(crate) grounded: bool,
}
