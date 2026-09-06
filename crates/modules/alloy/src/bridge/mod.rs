use crate::context::ExecutionPhase;
use rhai::Engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseCapabilities {
    pub validation_helpers: bool,
    pub db_services: bool,
    pub external_services: bool,
}

impl PhaseCapabilities {
    pub const fn for_phase(phase: ExecutionPhase) -> Self {
        match phase {
            ExecutionPhase::Before => Self {
                validation_helpers: true,
                db_services: false,
                external_services: false,
            },
            ExecutionPhase::After => Self {
                validation_helpers: false,
                db_services: true,
                external_services: false,
            },
            ExecutionPhase::OnCommit => Self {
                validation_helpers: false,
                db_services: false,
                external_services: true,
            },
            ExecutionPhase::Manual | ExecutionPhase::Scheduled => Self {
                validation_helpers: true,
                db_services: true,
                external_services: true,
            },
        }
    }
}

pub struct Bridge;

impl Bridge {
    pub fn capabilities_for_phase(phase: ExecutionPhase) -> PhaseCapabilities {
        PhaseCapabilities::for_phase(phase)
    }

    pub fn register_for_phase(engine: &mut Engine, phase: ExecutionPhase) {
        rustok_sandbox::rhai::register_standard_library(engine, sandbox_phase(phase));
    }
}

fn sandbox_phase(phase: ExecutionPhase) -> rustok_sandbox::ExecutionPhase {
    match phase {
        ExecutionPhase::Before => rustok_sandbox::ExecutionPhase::BeforeHook,
        ExecutionPhase::After => rustok_sandbox::ExecutionPhase::AfterHook,
        ExecutionPhase::OnCommit => rustok_sandbox::ExecutionPhase::Event,
        ExecutionPhase::Manual => rustok_sandbox::ExecutionPhase::Manual,
        ExecutionPhase::Scheduled => rustok_sandbox::ExecutionPhase::Scheduled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_capabilities_are_explicit() {
        assert_eq!(
            Bridge::capabilities_for_phase(ExecutionPhase::Before),
            PhaseCapabilities {
                validation_helpers: true,
                db_services: false,
                external_services: false,
            }
        );
        assert_eq!(
            Bridge::capabilities_for_phase(ExecutionPhase::After),
            PhaseCapabilities {
                validation_helpers: false,
                db_services: true,
                external_services: false,
            }
        );
        assert_eq!(
            Bridge::capabilities_for_phase(ExecutionPhase::OnCommit),
            PhaseCapabilities {
                validation_helpers: false,
                db_services: false,
                external_services: true,
            }
        );
        assert_eq!(
            Bridge::capabilities_for_phase(ExecutionPhase::Manual),
            PhaseCapabilities {
                validation_helpers: true,
                db_services: true,
                external_services: true,
            }
        );
        assert_eq!(
            Bridge::capabilities_for_phase(ExecutionPhase::Scheduled),
            PhaseCapabilities {
                validation_helpers: true,
                db_services: true,
                external_services: true,
            }
        );
    }
}
