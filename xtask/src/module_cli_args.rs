use super::*;

fn cli_option_value(args: &[String], flag: &str) -> Option<String> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1).cloned()
}

pub(crate) fn registry_url_argument(args: &[String]) -> Option<String> {
    cli_option_value(args, "--registry-url")
        .or_else(|| std::env::var("RUSTOK_MODULE_REGISTRY_URL").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn runner_token_argument(args: &[String]) -> Option<String> {
    cli_option_value(args, "--runner-token")
        .or_else(|| std::env::var(REMOTE_RUNNER_TOKEN_ENV).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) const MODULE_AUTH_TOKEN_ENV: &str = "RUSTOK_MODULE_AUTH_TOKEN";

pub(crate) fn positive_u64_argument(
    args: &[String],
    flag: &str,
    command_label: &str,
) -> Result<Option<u64>> {
    let Some(value) = cli_option_value(args, flag) else {
        return Ok(None);
    };
    let parsed = value.parse::<u64>().with_context(|| {
        format!("{command_label} flag {flag} expects a positive integer, got '{value}'")
    })?;
    if parsed == 0 {
        anyhow::bail!("{command_label} flag {flag} expects a positive integer, got '0'");
    }
    Ok(Some(parsed))
}

pub(crate) fn auth_token_argument(args: &[String]) -> Option<String> {
    cli_option_value(args, "--auth-token")
        .or_else(|| std::env::var(MODULE_AUTH_TOKEN_ENV).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn auto_approve_argument(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--auto-approve")
}

pub(crate) fn approve_reason_argument(args: &[String]) -> Option<String> {
    cli_option_value(args, "--approve-reason")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn manual_review_confirmation_argument(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--confirm-manual-review")
}

pub(crate) fn reason_argument(args: &[String]) -> Option<String> {
    cli_option_value(args, "--reason")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn detail_argument(args: &[String]) -> Option<String> {
    cli_option_value(args, "--detail")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn supported_remote_runner_stages(confirm_manual_review: bool) -> Vec<&'static str> {
    let mut stages = vec!["compile_smoke", "targeted_tests"];
    if confirm_manual_review {
        stages.push("security_policy_review");
    }
    stages
}

pub(crate) fn reason_code_argument(args: &[String]) -> Result<Option<String>> {
    let Some(reason_code) = cli_option_value(args, "--reason-code") else {
        return Ok(None);
    };
    let reason_code = reason_code.trim().to_ascii_lowercase();
    if reason_code.is_empty() {
        anyhow::bail!(
            "--reason-code expects one of: {}",
            REGISTRY_YANK_REASON_CODES.join("|")
        );
    }
    if !REGISTRY_YANK_REASON_CODES.contains(&reason_code.as_str()) {
        anyhow::bail!(
            "--reason-code '{}' is invalid; expected one of: {}",
            reason_code,
            REGISTRY_YANK_REASON_CODES.join("|")
        );
    }
    Ok(Some(reason_code.to_string()))
}

pub(crate) fn owner_transfer_reason_code_argument(args: &[String]) -> Result<Option<String>> {
    normalized_reason_code_argument(
        args,
        REGISTRY_OWNER_TRANSFER_REASON_CODES,
        "module owner-transfer",
        "--reason-code",
    )
}

pub(crate) fn approve_reason_code_argument(args: &[String]) -> Result<Option<String>> {
    let Some(reason_code) = cli_option_value(args, "--approve-reason-code") else {
        return Ok(None);
    };
    let reason_code = reason_code.trim().to_ascii_lowercase();
    if reason_code.is_empty() {
        anyhow::bail!(
            "--approve-reason-code expects one of: {}",
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join("|")
        );
    }
    if !REGISTRY_APPROVE_OVERRIDE_REASON_CODES.contains(&reason_code.as_str()) {
        anyhow::bail!(
            "--approve-reason-code '{}' is invalid; expected one of: {}",
            reason_code,
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES.join("|")
        );
    }
    Ok(Some(reason_code.to_string()))
}

pub(crate) fn validation_stage_reason_code_argument(args: &[String]) -> Result<Option<String>> {
    normalized_reason_code_argument(
        args,
        REGISTRY_VALIDATION_STAGE_REASON_CODES,
        "module stage",
        "--reason-code",
    )
}

pub(crate) fn governance_reason_code_argument(
    args: &[String],
    command_name: &str,
    allowed_reason_codes: &[&str],
) -> Result<Option<String>> {
    let Some(reason_code) = cli_option_value(args, "--reason-code") else {
        return Ok(None);
    };
    let reason_code = reason_code.trim().to_ascii_lowercase();
    if reason_code.is_empty() {
        anyhow::bail!(
            "module {command_name} --reason-code expects one of: {}",
            allowed_reason_codes.join("|")
        );
    }
    if !allowed_reason_codes.contains(&reason_code.as_str()) {
        anyhow::bail!(
            "module {command_name} --reason-code '{}' is invalid; expected one of: {}",
            reason_code,
            allowed_reason_codes.join("|")
        );
    }
    Ok(Some(reason_code.to_string()))
}

fn normalized_reason_code_argument(
    args: &[String],
    allowed: &[&str],
    command_label: &str,
    flag: &str,
) -> Result<Option<String>> {
    let Some(value) = cli_option_value(args, flag) else {
        return Ok(None);
    };
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(None);
    }
    if !allowed.iter().any(|candidate| *candidate == value) {
        anyhow::bail!(
            "{} reason code '{}' is not supported; expected one of {}",
            command_label,
            value,
            allowed.join(", ")
        );
    }
    Ok(Some(value))
}
