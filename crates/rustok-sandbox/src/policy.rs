use serde::{Deserialize, Serialize};

use crate::CapabilityGrant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxLimits {
    pub wall_clock_ms: u64,
    pub instruction_budget: u64,
    pub max_memory_bytes: u64,
    pub max_output_bytes: u64,
    pub max_concurrency: u32,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            wall_clock_ms: 100,
            instruction_budget: 50_000,
            max_memory_bytes: 64 * 1024 * 1024,
            max_output_bytes: 1024 * 1024,
            max_concurrency: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxPolicy {
    #[serde(default)]
    pub grants: Vec<CapabilityGrant>,
    #[serde(default)]
    pub limits: SandboxLimits,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            grants: Vec::new(),
            limits: SandboxLimits::default(),
        }
    }
}

impl SandboxPolicy {
    pub fn grant(&self, name: &crate::CapabilityName) -> Option<&CapabilityGrant> {
        self.grants.iter().find(|grant| &grant.name == name)
    }
}

