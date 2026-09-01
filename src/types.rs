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
