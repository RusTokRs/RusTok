#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleOperationStatus {
    Validated,
    Running,
    /// The state transition committed durably, but its post-hook is still
    /// outstanding. This is not terminal: the executor must record either
    /// `Committed` after the hook succeeds or `CommittedHookFailed` after it
    /// fails. The stale-operation reaper uses this distinct state to recover
    /// an owner that disappeared after the state commit without rewriting
    /// already-completed `Committed` operations.
    CommittedHookPending,
    /// The state transition and every required post-hook completed
    /// successfully.
    Committed,
    /// The state transition committed durably, but the post-hook failed.
    /// The operation must never be reported as `Failed`: its state change is
    /// in effect, and only the hook remains outstanding (recovery retries it
    /// via `ModuleOperationRecoveryAction::RetryPostHook`).
    CommittedHookFailed,
    Failed,
    /// The executor disappeared before a durable state commit was recorded.
    /// The operation is terminal and must be repeated as a new command after
    /// rereading the aggregate; it is not a hook or persistence failure.
    Orphaned,
}

impl ModuleOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validated => "validated",
            Self::Running => "running",
            Self::CommittedHookPending => "committed_hook_pending",
            Self::Committed => "committed",
            Self::CommittedHookFailed => "committed_hook_failed",
            Self::Failed => "failed",
            Self::Orphaned => "orphaned",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Committed | Self::Failed | Self::Orphaned)
    }

    /// Whether the operation's durable state change is in effect.
    pub const fn state_committed(self) -> bool {
        matches!(
            self,
            Self::CommittedHookPending | Self::Committed | Self::CommittedHookFailed
        )
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "validated" => Some(Self::Validated),
            "running" => Some(Self::Running),
            "committed_hook_pending" => Some(Self::CommittedHookPending),
            "committed" => Some(Self::Committed),
            "committed_hook_failed" => Some(Self::CommittedHookFailed),
            "failed" => Some(Self::Failed),
            "orphaned" => Some(Self::Orphaned),
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
    Orphaned,
}

impl ModuleOperationIssue {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PreHookFailed => "pre_hook_failed",
            Self::PostHookFailed => "post_hook_failed",
            Self::OtherFailed => "other_failed",
            Self::Orphaned => "orphaned",
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
