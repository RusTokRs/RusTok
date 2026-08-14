//! Remote-only MCP contracts for tenant-bound Alloy script authoring.
//!
//! These tool names are intentionally absent from the generic stdio and
//! in-process MCP server. A host can advertise them only after it resolves a
//! durable remote binding and constructs `alloy::AlloyAuthoringService` from
//! the matching owner-scoped runtime.

pub use alloy::{
    AlloyExecutionOutcome, AlloyScriptLifecycleAction, AlloyScriptValidation, AuthoringEntityInput,
    ChangeAlloyScriptLifecycleCommand, CreateAlloyScriptCommand, DeleteAlloyScriptCommand,
    DeletedAlloyScript, GetAlloyScriptCommand, ListAlloyScriptReviewsCommand,
    ListAlloyScriptRevisionsCommand, ListAlloyScriptsCommand, RedactedAlloyExecution,
    RedactedAlloyReview, RedactedAlloyScript, RedactedAlloyScriptPage, RedactedAlloySourceRevision,
    RedactedAlloyTestRun, ReviewAlloyScriptCommand, RunAlloyScriptCommand,
    RunAlloyWorkspaceTestCommand, UpdateAlloyScriptCommand, ValidateAlloyScriptCommand,
};

pub const TOOL_ALLOY_LIST_SCRIPTS: &str = "alloy_list_scripts";
pub const TOOL_ALLOY_GET_SCRIPT: &str = "alloy_get_script";
pub const TOOL_ALLOY_LIST_SCRIPT_REVISIONS: &str = "alloy_list_script_revisions";
pub const TOOL_ALLOY_CREATE_SCRIPT: &str = "alloy_create_script";
pub const TOOL_ALLOY_UPDATE_SCRIPT: &str = "alloy_update_script";
pub const TOOL_ALLOY_DELETE_SCRIPT: &str = "alloy_delete_script";
pub const TOOL_ALLOY_VALIDATE_SCRIPT: &str = "alloy_validate_script";
pub const TOOL_ALLOY_RUN_SCRIPT: &str = "alloy_run_script";
pub const TOOL_ALLOY_REVIEW_SCRIPT: &str = "alloy_review_script";
pub const TOOL_ALLOY_LIST_SCRIPT_REVIEWS: &str = "alloy_list_script_reviews";
pub const TOOL_ALLOY_RUN_WORKSPACE_TEST: &str = "alloy_run_workspace_test";
pub const TOOL_ALLOY_CHANGE_SCRIPT_LIFECYCLE: &str = "alloy_change_script_lifecycle";

/// The only remote MCP names that may invoke source-bearing Alloy authoring.
/// The normal MCP server deliberately does not consume this array.
pub const REMOTE_ALLOY_AUTHORING_TOOL_NAMES: &[&str] = &[
    TOOL_ALLOY_LIST_SCRIPTS,
    TOOL_ALLOY_GET_SCRIPT,
    TOOL_ALLOY_LIST_SCRIPT_REVISIONS,
    TOOL_ALLOY_CREATE_SCRIPT,
    TOOL_ALLOY_UPDATE_SCRIPT,
    TOOL_ALLOY_DELETE_SCRIPT,
    TOOL_ALLOY_VALIDATE_SCRIPT,
    TOOL_ALLOY_RUN_SCRIPT,
    TOOL_ALLOY_REVIEW_SCRIPT,
    TOOL_ALLOY_LIST_SCRIPT_REVIEWS,
    TOOL_ALLOY_RUN_WORKSPACE_TEST,
    TOOL_ALLOY_CHANGE_SCRIPT_LIFECYCLE,
];

pub fn is_remote_alloy_authoring_tool(tool_name: &str) -> bool {
    REMOTE_ALLOY_AUTHORING_TOOL_NAMES.contains(&tool_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ALL_ALLOY_TOOLS;

    #[test]
    fn source_bearing_authoring_tools_are_remote_only() {
        assert!(is_remote_alloy_authoring_tool(TOOL_ALLOY_CREATE_SCRIPT));
        assert!(is_remote_alloy_authoring_tool(TOOL_ALLOY_RUN_SCRIPT));
        for tool in REMOTE_ALLOY_AUTHORING_TOOL_NAMES {
            assert!(
                !ALL_ALLOY_TOOLS.contains(tool),
                "generic MCP must not advertise remote-only Alloy authoring tool {tool}"
            );
        }
    }
}
