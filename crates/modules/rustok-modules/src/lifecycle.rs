#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleOperationStatus {
    Validated,
    Running,
    Committed,
    Failed,
}

impl ModuleOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validated => "validated",
            Self::Running => "running",
            Self::Committed => "committed",
            Self::Failed => "failed",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Committed | Self::Failed)
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "validated" => Some(Self::Validated),
            "running" => Some(Self::Running),
            "committed" => Some(Self::Committed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

impl std::fmt::Display for ModuleOperationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ModuleOperationStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}

impl From<ModuleOperationStatus> for String {
    fn from(value: ModuleOperationStatus) -> Self {
        value.as_str().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleOperationIssue {
    None,
    PreHookFailed,
    PostHookFailed,
    OtherFailed,
}

impl ModuleOperationIssue {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PreHookFailed => "pre_hook_failed",
            Self::PostHookFailed => "post_hook_failed",
            Self::OtherFailed => "other_failed",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(self, Self::PostHookFailed)
    }
}

impl std::fmt::Display for ModuleOperationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleOperationRecoveryAction {
    None,
    RetryPostHook,
    RepeatToggle,
    CompensatingToggle,
}

impl ModuleOperationRecoveryAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RetryPostHook => "retry_post_hook",
            Self::RepeatToggle => "repeat_toggle",
            Self::CompensatingToggle => "compensating_toggle",
        }
    }
}
