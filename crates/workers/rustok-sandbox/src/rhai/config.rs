use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RhaiConfig {
    pub max_operations: u64,
    pub timeout: Duration,
    pub max_call_depth: usize,
    pub max_string_size: usize,
    pub max_array_size: usize,
    pub max_map_size: usize,
}

impl Default for RhaiConfig {
    fn default() -> Self {
        Self {
            max_operations: 50_000,
            timeout: Duration::from_millis(100),
            max_call_depth: 16,
            max_string_size: 64 * 1024,
            max_array_size: 10_000,
            max_map_size: 16,
        }
    }
}

impl RhaiConfig {
    pub fn relaxed() -> Self {
        Self {
            max_operations: 500_000,
            timeout: Duration::from_secs(5),
            ..Default::default()
        }
    }

    pub fn strict() -> Self {
        Self {
            max_operations: 10_000,
            timeout: Duration::from_millis(50),
            max_call_depth: 8,
            ..Default::default()
        }
    }

    pub fn limits(&self) -> RhaiLimits {
        RhaiLimits {
            max_operations: self.max_operations,
            timeout_ms: self.timeout.as_millis().try_into().unwrap_or(u64::MAX),
            max_call_depth: self.max_call_depth,
            max_string_size: self.max_string_size,
            max_array_size: self.max_array_size,
            max_map_size: self.max_map_size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RhaiLimits {
    pub max_operations: u64,
    pub timeout_ms: u64,
    pub max_call_depth: usize,
    pub max_string_size: usize,
    pub max_array_size: usize,
    pub max_map_size: usize,
}
