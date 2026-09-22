use super::*;

pub(crate) fn normalize_executable_validation_stage(stage: &str) -> Result<&'static str> {
    let stage = stage.trim().to_ascii_lowercase();
    match stage.as_str() {
        "compile_smoke" => Ok("compile_smoke"),
        "targeted_tests" => Ok("targeted_tests"),
        "security_policy_review" => Ok("security_policy_review"),
        _ => anyhow::bail!(
            "module stage-run stage '{}' is not supported; expected one of compile_smoke, targeted_tests, or security_policy_review",
            stage
        ),
    }
}

pub(crate) fn executable_validation_stage_running_detail(stage: &str, slug: &str) -> String {
    match stage {
        "compile_smoke" => format!("Running local compile smoke checks for module '{slug}'."),
        "targeted_tests" => format!("Running local targeted tests for module '{slug}'."),
        "security_policy_review" => {
            format!("Running local security/policy review preflight for module '{slug}'.")
        }
        _ => format!("Running local validation stage '{stage}' for module '{slug}'."),
    }
}

pub(crate) fn executable_validation_stage_success_detail(stage: &str, slug: &str) -> String {
    match stage {
        "compile_smoke" => {
            format!("Local compile smoke checks passed for module '{slug}'.")
        }
        "targeted_tests" => {
            format!("Local targeted tests completed successfully for module '{slug}'.")
        }
        "security_policy_review" => format!(
            "Local security/policy preflight completed and manual review was confirmed for module '{slug}'."
        ),
        _ => format!("Local validation stage '{stage}' completed successfully for '{slug}'."),
    }
}

pub(crate) fn executable_validation_stage_failure_prefix(stage: &str) -> String {
    match stage {
        "compile_smoke" => "Local compile smoke failed".to_string(),
        "targeted_tests" => "Local targeted tests failed".to_string(),
        "security_policy_review" => "Local security/policy review preflight failed".to_string(),
        _ => format!("Local validation stage '{stage}' failed"),
    }
}

pub(crate) fn validation_stage_success_reason_code(stage: &str) -> &'static str {
    match stage {
        "security_policy_review" => "manual_review_complete",
        _ => "local_runner_passed",
    }
}

pub(crate) fn validation_stage_failure_reason_code(stage: &str) -> &'static str {
    match stage {
        "compile_smoke" => "build_failure",
        "targeted_tests" => "test_failure",
        "security_policy_review" => "policy_preflight_failed",
        _ => "other",
    }
}

pub(crate) fn validation_stage_status_requires_reason_code(status: &str) -> bool {
    matches!(status, "passed" | "failed" | "blocked")
}
