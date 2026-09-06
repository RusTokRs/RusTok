use serde::{Deserialize, Serialize};

use crate::CapabilityGrant;

const fn default_max_capability_calls() -> u32 {
    16
}

const fn default_max_capability_input_bytes() -> u64 {
    64 * 1024
}

const fn default_max_capability_calls_per_second() -> u32 {
    16
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxLimits {
    pub wall_clock_ms: u64,
    pub instruction_budget: u64,
    pub max_memory_bytes: u64,
    pub max_output_bytes: u64,
    pub max_concurrency: u32,
    #[serde(default = "default_max_capability_calls")]
    pub max_capability_calls: u32,
    #[serde(default = "default_max_capability_input_bytes")]
    pub max_capability_input_bytes: u64,
    #[serde(default = "default_max_capability_calls_per_second")]
    pub max_capability_calls_per_second: u32,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            wall_clock_ms: 100,
            instruction_budget: 50_000,
            max_memory_bytes: 64 * 1024 * 1024,
            max_output_bytes: 1024 * 1024,
            max_concurrency: 1,
            max_capability_calls: default_max_capability_calls(),
            max_capability_input_bytes: default_max_capability_input_bytes(),
            max_capability_calls_per_second: default_max_capability_calls_per_second(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SandboxPolicy {
    #[serde(default)]
    pub grants: Vec<CapabilityGrant>,
    #[serde(default)]
    pub limits: SandboxLimits,
}

impl SandboxPolicy {
    pub fn grant(&self, name: &crate::CapabilityName) -> Option<&CapabilityGrant> {
        self.grants.iter().find(|grant| &grant.name == name)
    }
}
