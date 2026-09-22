use super::*;

pub(crate) fn build_module_test_plan(
    preview: &ModulePublishDryRunPreview,
) -> ModuleTestPlanPreview {
    let mut commands = vec![ModuleCommandPreview {
        label: "module validate".to_string(),
        argv: vec![
            "cargo".to_string(),
            "run".to_string(),
            "-p".to_string(),
            "xtask".to_string(),
            "--".to_string(),
            "module".to_string(),
            "validate".to_string(),
            preview.slug.clone(),
        ],
    }];

    commands.push(package_check_command("module crate", &preview.crate_name));

    if let Some(admin) = preview.ui_packages.admin.as_ref() {
        commands.push(package_check_command("admin ui crate", &admin.crate_name));
    }

    if let Some(storefront) = preview.ui_packages.storefront.as_ref() {
        commands.push(package_check_command(
            "storefront ui crate",
            &storefront.crate_name,
        ));
    }

    ModuleTestPlanPreview {
        slug: preview.slug.clone(),
        version: preview.version.clone(),
        commands,
    }
}

pub(crate) fn build_module_validation_stage_run_preview(
    preview: &ModulePublishDryRunPreview,
    request_id: &str,
    stage: &str,
    detail_override: Option<String>,
) -> Result<ModuleValidationStageRunPreview> {
    let normalized_stage = normalize_executable_validation_stage(stage)?;
    let (commands, requires_manual_confirmation) = match normalized_stage {
        "compile_smoke" => (build_compile_smoke_commands(preview), false),
        "targeted_tests" => (build_module_test_plan(preview).commands, false),
        "security_policy_review" => (build_security_policy_review_commands(preview), true),
        _ => unreachable!(),
    };
    let running_detail = detail_override.unwrap_or_else(|| {
        executable_validation_stage_running_detail(normalized_stage, &preview.slug)
    });

    Ok(ModuleValidationStageRunPreview {
        action: "validation_stage_run".to_string(),
        slug: preview.slug.clone(),
        request_id: request_id.to_string(),
        stage: normalized_stage.to_string(),
        requires_manual_confirmation,
        running_detail,
        success_detail: executable_validation_stage_success_detail(normalized_stage, &preview.slug),
        failure_detail_prefix: executable_validation_stage_failure_prefix(normalized_stage),
        commands,
    })
}

fn build_compile_smoke_commands(preview: &ModulePublishDryRunPreview) -> Vec<ModuleCommandPreview> {
    let mut commands = vec![package_check_command("module crate", &preview.crate_name)];

    if let Some(admin) = preview.ui_packages.admin.as_ref() {
        commands.push(package_check_command("admin ui crate", &admin.crate_name));
    }

    if let Some(storefront) = preview.ui_packages.storefront.as_ref() {
        commands.push(package_check_command(
            "storefront ui crate",
            &storefront.crate_name,
        ));
    }

    commands
}

fn build_security_policy_review_commands(
    preview: &ModulePublishDryRunPreview,
) -> Vec<ModuleCommandPreview> {
    vec![
        ModuleCommandPreview {
            label: "module validate".to_string(),
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "xtask".to_string(),
                "--".to_string(),
                "module".to_string(),
                "validate".to_string(),
                preview.slug.clone(),
            ],
        },
        ModuleCommandPreview {
            label: "module test dry-run".to_string(),
            argv: vec![
                "cargo".to_string(),
                "run".to_string(),
                "-p".to_string(),
                "xtask".to_string(),
                "--".to_string(),
                "module".to_string(),
                "test".to_string(),
                preview.slug.clone(),
                "--dry-run".to_string(),
            ],
        },
    ]
}

fn package_check_command(label: &str, package: &str) -> ModuleCommandPreview {
    ModuleCommandPreview {
        label: label.to_string(),
        argv: vec![
            "cargo".to_string(),
            "check".to_string(),
            "-p".to_string(),
            package.to_string(),
        ],
    }
}
