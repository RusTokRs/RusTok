use std::fs;
use std::path::Path;

#[test]
fn native_module_composition_endpoints_are_not_declared() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for endpoint in [
        "endpoint = \"admin/install-module\"",
        "endpoint = \"admin/uninstall-module\"",
        "endpoint = \"admin/upgrade-module\"",
        "endpoint = \"admin/toggle-module\"",
    ] {
        assert!(
            !content.contains(endpoint),
            "Forbidden native module composition endpoint found: {endpoint}"
        );
    }
}


#[test]
fn module_composition_helpers_do_not_use_raw_sql_for_platform_state_or_builds() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for forbidden in [
        "UPDATE platform_state",
        "INSERT INTO builds",
        "INSERT INTO module_operations",
        "SELECT revision FROM platform_state",
        "save_manifest_and_enqueue_build",
    ] {
        assert!(
            !content.contains(forbidden),
            "Forbidden raw SQL/platform composition helper fragment found in api.rs: {forbidden}"
        );
    }
}

#[test]
fn module_composition_client_helpers_do_not_call_native_paths() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for native_call in [
        "install_module_native(",
        "uninstall_module_native(",
        "upgrade_module_native(",
        "toggle_module_native(",
    ] {
        assert!(
            !content.contains(native_call),
            "Forbidden native composition call path found: {native_call}"
        );
    }
}

#[test]
fn module_composition_helpers_do_not_use_native_graphql_fallback_combiner() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for helper in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, helper)
            .unwrap_or_else(|| panic!("helper signature not found: {helper}"));

        assert!(
            !helper_body.contains("combine_native_and_graphql_error"),
            "Forbidden native/graphql fallback combiner found in module composition helper: {helper}"
        );
    }
}

#[test]
fn module_composition_helpers_use_graphql_contract_payloads() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    assert_graphql_only_helper(
        &content,
        "pub async fn install_module(",
        "INSTALL_MODULE_MUTATION",
        "InstallModuleVariables {",
        &["slug,", "version,"],
        "Ok(response.install_module)",
    );
    assert_graphql_only_helper(
        &content,
        "pub async fn uninstall_module(",
        "UNINSTALL_MODULE_MUTATION",
        "UninstallModuleVariables {",
        &["slug,"],
        "Ok(response.uninstall_module)",
    );
    assert_graphql_only_helper(
        &content,
        "pub async fn upgrade_module(",
        "UPGRADE_MODULE_MUTATION",
        "UpgradeModuleVariables {",
        &["slug,", "version,"],
        "Ok(response.upgrade_module)",
    );
    assert_graphql_only_helper(
        &content,
        "pub async fn toggle_module(",
        "TOGGLE_MODULE_MUTATION",
        "ToggleModuleVariables {",
        &["module_slug,", "enabled,"],
        "Ok(response.toggle_module)",
    );
}


#[test]
fn module_composition_helpers_forward_auth_context_without_local_overrides() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        assert_eq!(
            helper_body.matches("request(").count(),
            1,
            "{signature} must perform exactly one GraphQL request call"
        );
        assert!(
            helper_body.contains("token,"),
            "{signature} must forward token to canonical GraphQL request"
        );
        assert!(
            helper_body.contains("tenant_slug,"),
            "{signature} must forward tenant_slug to canonical GraphQL request"
        );
        assert!(
            !helper_body.contains(".map_err("),
            "{signature} must not remap canonical GraphQL ApiError taxonomy"
        );
        assert!(
            !helper_body.contains("Some("),
            "{signature} must not locally override auth context when forwarding request"
        );
        assert!(
            !helper_body.contains("None"),
            "{signature} must not locally null auth context when forwarding request"
        );
    }
}

#[test]
fn module_composition_helpers_do_not_branch_on_runtime_error_taxonomy() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for forbidden in [
            "UNKNOWN_MODULE",
            "CORE_MODULE",
            "MISSING_DEPENDENCIES",
            "HAS_DEPENDENTS",
            "MODULE_HOOK_FAILED",
            "extensions.code",
            "reason_code",
            "module_operations",
            "correlation_id",
            "requested_by",
            "ApiError::GraphQl",
            "GraphQlError",
            "graphQLErrors",
        ] {
            assert!(
                !helper_body.contains(forbidden),
                "{signature} must not branch on runtime taxonomy fragment `{forbidden}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_implement_local_retry_or_compensation_flows() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for forbidden in [
            "for attempt in",
            "loop {",
            "retry",
            "compensat",
            "module_operations",
            "correlation_id",
        ] {
            assert!(
                !helper_body.contains(forbidden),
                "{signature} must not introduce local retry/compensation logic fragment `{forbidden}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_preserve_server_owned_lifecycle_parity_matrix_contract() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let lifecycle_taxonomy_fragments = [
        "UNKNOWN_MODULE",
        "CORE_MODULE",
        "MISSING_DEPENDENCIES",
        "HAS_DEPENDENTS",
        "MODULE_HOOK_FAILED",
    ];
    let journal_metadata_fragments = [
        "module_operations",
        "correlation_id",
        "requested_by",
        "previous_effective_enabled",
        "retryable",
    ];
    let composition_error_fragments = [
        "REVISION_CONFLICT",
        "INVALID_MODULE",
        "REQUIRED_MODULE",
        "UNKNOWN_DEPENDENCY",
        "INTERNAL_ERROR",
        "manifest_ref",
        "platform_state:",
    ];

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for fragment in lifecycle_taxonomy_fragments
            .iter()
            .chain(journal_metadata_fragments.iter())
            .chain(composition_error_fragments.iter())
        {
            assert!(
                !helper_body.contains(fragment),
                "{signature} must keep server-owned parity contract and not parse fragment `{fragment}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_parse_lifecycle_operation_status_taxonomy() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let status_fragments = [
        "validated",
        "running",
        "committed",
        "failed",
        "status",
        "operation",
        "retryable_issue",
    ];

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for fragment in status_fragments {
            assert!(
                !helper_body.contains(fragment),
                "{signature} must not parse lifecycle operation status fragment `{fragment}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_parse_manifest_ref_or_revision_contract() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let manifest_contract_fragments = [
        "manifest_ref",
        "platform_state:",
        "manifest_revision",
        "expected_revision",
        "revision",
    ];

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for fragment in manifest_contract_fragments {
            assert!(
                !helper_body.contains(fragment),
                "{signature} must not parse server-owned manifest contract fragment `{fragment}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_branch_on_control_plane_error_taxonomy() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let control_plane_error_fragments = [
        "CONFLICT",
        "VALIDATION",
        "INTERNAL",
        "stale revision",
        "expected_revision",
        "ApiError::BadRequest",
        "ApiError::ServerError",
    ];

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for fragment in control_plane_error_fragments {
            assert!(
                !helper_body.contains(fragment),
                "{signature} must not locally branch on control-plane error taxonomy fragment `{fragment}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_parse_build_or_release_pipeline_contract() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let pipeline_fragments = [
        "builds",
        "build_id",
        "build_status",
        "release",
        "active_release_id",
        "manifest_hash",
        "manifest_snapshot",
        "modules_delta",
    ];

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for fragment in pipeline_fragments {
            assert!(
                !helper_body.contains(fragment),
                "{signature} must not parse build/release pipeline fragment `{fragment}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_parse_graphql_error_payload_shapes() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let graphql_error_shape_fragments = [
        ".errors",
        "graphQLErrors",
        "extensions",
        "reason_code",
        "message.as_str()",
        "errors.first()",
    ];

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for fragment in graphql_error_shape_fragments {
            assert!(
                !helper_body.contains(fragment),
                "{signature} must not parse GraphQL error payload shape fragment `{fragment}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_map_graphql_taxonomy_to_transport_error_variants() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let forbidden_transport_remap_fragments = [
        "ApiError::Unauthorized",
        "ApiError::Network",
        "ApiError::Http(",
        "\"Unauthorized\"",
        "\"Network error\"",
        "\"Http error: \"",
    ];

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for fragment in forbidden_transport_remap_fragments {
            assert!(
                !helper_body.contains(fragment),
                "{signature} must not remap GraphQL taxonomy into transport fragment `{fragment}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_use_local_serverfn_error_normalizers() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let forbidden_local_normalizer_fragments = [
        "normalize_server_fn_error_message(",
        "map_server_fn_error(",
        "ServerFnError::new(",
    ];

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        for fragment in forbidden_local_normalizer_fragments {
            assert!(
                !helper_body.contains(fragment),
                "{signature} must not implement local ServerFnError normalization fragment `{fragment}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_do_not_cross_wire_foreign_mutation_contracts() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let cases = [
        (
            "pub async fn install_module(",
            "INSTALL_MODULE_MUTATION",
            [
                "UNINSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
        ),
        (
            "pub async fn uninstall_module(",
            "UNINSTALL_MODULE_MUTATION",
            [
                "INSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
        ),
        (
            "pub async fn upgrade_module(",
            "UPGRADE_MODULE_MUTATION",
            [
                "INSTALL_MODULE_MUTATION",
                "UNINSTALL_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
        ),
        (
            "pub async fn toggle_module(",
            "TOGGLE_MODULE_MUTATION",
            [
                "INSTALL_MODULE_MUTATION",
                "UNINSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
            ],
        ),
    ];

    for (signature, required, forbidden_list) in cases {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));
        assert!(
            helper_body.contains(required),
            "{signature} must reference canonical mutation constant `{required}`"
        );
        for forbidden in forbidden_list {
            assert!(
                !helper_body.contains(forbidden),
                "{signature} must not cross-wire foreign mutation constant `{forbidden}`"
            );
        }
    }
}

#[test]
fn module_composition_helpers_use_typed_responses_and_direct_payload_returns() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let cases = [
        (
            "pub async fn install_module(",
            "let response: InstallModuleResponse",
            "Ok(response.install_module)",
            [
                "response.uninstall_module",
                "response.upgrade_module",
                "response.toggle_module",
            ],
        ),
        (
            "pub async fn uninstall_module(",
            "let response: UninstallModuleResponse",
            "Ok(response.uninstall_module)",
            [
                "response.install_module",
                "response.upgrade_module",
                "response.toggle_module",
            ],
        ),
        (
            "pub async fn upgrade_module(",
            "let response: UpgradeModuleResponse",
            "Ok(response.upgrade_module)",
            [
                "response.install_module",
                "response.uninstall_module",
                "response.toggle_module",
            ],
        ),
        (
            "pub async fn toggle_module(",
            "let response: ToggleModuleResponse",
            "Ok(response.toggle_module)",
            [
                "response.install_module",
                "response.uninstall_module",
                "response.upgrade_module",
            ],
        ),
    ];

    for (signature, typed_response, canonical_return, forbidden_returns) in cases {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        assert!(
            helper_body.contains(typed_response),
            "{signature} must decode GraphQL response into `{typed_response}`"
        );
        assert!(
            helper_body.contains(canonical_return),
            "{signature} must return canonical payload `{canonical_return}`"
        );
        for forbidden in forbidden_returns {
            assert!(
                !helper_body.contains(forbidden),
                "{signature} must not return foreign payload fragment `{forbidden}`"
            );
        }
    }
}

#[test]
fn module_composition_mutation_constants_are_declared_once() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for constant in [
        "pub const INSTALL_MODULE_MUTATION: &str =",
        "pub const UNINSTALL_MODULE_MUTATION: &str =",
        "pub const UPGRADE_MODULE_MUTATION: &str =",
        "pub const TOGGLE_MODULE_MUTATION: &str =",
    ] {
        let occurrences = content.matches(constant).count();
        assert_eq!(
            occurrences, 1,
            "expected exactly one mutation constant declaration `{constant}`, found {occurrences}"
        );
    }
}

#[test]
fn module_composition_helpers_reference_single_canonical_mutation_and_request_call() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let cases = [
        (
            "pub async fn install_module(",
            "INSTALL_MODULE_MUTATION",
            ["UNINSTALL_MODULE_MUTATION", "UPGRADE_MODULE_MUTATION"],
        ),
        (
            "pub async fn uninstall_module(",
            "UNINSTALL_MODULE_MUTATION",
            ["INSTALL_MODULE_MUTATION", "UPGRADE_MODULE_MUTATION"],
        ),
        (
            "pub async fn upgrade_module(",
            "UPGRADE_MODULE_MUTATION",
            ["INSTALL_MODULE_MUTATION", "UNINSTALL_MODULE_MUTATION"],
        ),
        (
            "pub async fn toggle_module(",
            "TOGGLE_MODULE_MUTATION",
            [
                "INSTALL_MODULE_MUTATION",
                "UNINSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
            ],
        ),
    ];

    for (signature, canonical_mutation, foreign_mutations) in cases {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

        assert_eq!(
            helper_body.matches("request(").count(),
            1,
            "{signature} must call request exactly once"
        );
        assert_eq!(
            helper_body.matches(canonical_mutation).count(),
            1,
            "{signature} must reference canonical mutation constant exactly once"
        );
        for foreign in foreign_mutations {
            assert!(
                !helper_body.contains(foreign),
                "{signature} must not reference foreign mutation constant `{foreign}`"
            );
        }
        assert!(
            !helper_body.to_lowercase().contains("rolled back"),
            "{signature} must not encode rollback semantics in helper-level contracts"
        );
    }
}

#[test]
fn rollback_build_helper_is_the_only_module_api_helper_with_native_graphql_fallback_combiner() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let rollback_body = extract_function_block(&content, "pub async fn rollback_build(")
        .expect("rollback_build helper signature not found");
    assert!(
        rollback_body.contains("combine_native_and_graphql_error"),
        "rollback_build must preserve native/graphql fallback combiner contract"
    );
    assert!(
        rollback_body.contains("rollback_build_native("),
        "rollback_build must preserve native-first fallback path"
    );

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let helper_body = extract_function_block(&content, signature)
            .unwrap_or_else(|| panic!("helper signature not found: {signature}"));
        assert!(
            !helper_body.contains("combine_native_and_graphql_error"),
            "{signature} must not use native/graphql fallback combiner"
        );
    }
}

#[test]
fn module_composition_helpers_preserve_canonical_graphql_contract_matrix() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    struct Case<'a> {
        signature: &'a str,
        mutation: &'a str,
        typed_response: &'a str,
        canonical_return: &'a str,
        foreign_mutations: [&'a str; 3],
        required_payload_fields: &'a [&'a str],
    }

    let cases = [
        Case {
            signature: "pub async fn install_module(",
            mutation: "INSTALL_MODULE_MUTATION",
            typed_response: "let response: InstallModuleResponse",
            canonical_return: "Ok(response.install_module)",
            foreign_mutations: [
                "UNINSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
            required_payload_fields: &["InstallModuleVariables {", "slug,", "version,", "token,", "tenant_slug,"],
        },
        Case {
            signature: "pub async fn uninstall_module(",
            mutation: "UNINSTALL_MODULE_MUTATION",
            typed_response: "let response: UninstallModuleResponse",
            canonical_return: "Ok(response.uninstall_module)",
            foreign_mutations: [
                "INSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
            required_payload_fields: &["UninstallModuleVariables {", "slug,", "token,", "tenant_slug,"],
        },
        Case {
            signature: "pub async fn upgrade_module(",
            mutation: "UPGRADE_MODULE_MUTATION",
            typed_response: "let response: UpgradeModuleResponse",
            canonical_return: "Ok(response.upgrade_module)",
            foreign_mutations: [
                "INSTALL_MODULE_MUTATION",
                "UNINSTALL_MODULE_MUTATION",
                "TOGGLE_MODULE_MUTATION",
            ],
            required_payload_fields: &["UpgradeModuleVariables {", "slug,", "version,", "token,", "tenant_slug,"],
        },
        Case {
            signature: "pub async fn toggle_module(",
            mutation: "TOGGLE_MODULE_MUTATION",
            typed_response: "let response: ToggleModuleResponse",
            canonical_return: "Ok(response.toggle_module)",
            foreign_mutations: [
                "INSTALL_MODULE_MUTATION",
                "UNINSTALL_MODULE_MUTATION",
                "UPGRADE_MODULE_MUTATION",
            ],
            required_payload_fields: &[
                "ToggleModuleVariables {",
                "module_slug,",
                "enabled,",
                "token,",
                "tenant_slug,",
            ],
        },
    ];

    for case in cases {
        let helper_body = extract_function_block(&content, case.signature)
            .unwrap_or_else(|| panic!("helper signature not found: {}", case.signature));

        assert_eq!(helper_body.matches("request(").count(), 1, "{} must call request exactly once", case.signature);
        assert_eq!(helper_body.matches(case.mutation).count(), 1, "{} must reference canonical mutation exactly once", case.signature);
        assert!(helper_body.contains(case.typed_response), "{} must decode typed response", case.signature);
        assert!(helper_body.contains(case.canonical_return), "{} must return canonical payload", case.signature);

        for forbidden in case.foreign_mutations {
            assert!(
                !helper_body.contains(forbidden),
                "{} must not reference foreign mutation `{forbidden}`",
                case.signature
            );
        }
        for field in case.required_payload_fields {
            assert!(
                helper_body.contains(field),
                "{} must preserve GraphQL payload/auth forwarding field `{field}`",
                case.signature
            );
        }

        for forbidden_fragment in [
            ".map_err(",
            "combine_native_and_graphql_error",
            "Some(",
            "None",
            "UNKNOWN_MODULE",
            "CORE_MODULE",
            "HAS_DEPENDENTS",
            "MISSING_DEPENDENCIES",
            "MODULE_HOOK_FAILED",
            "extensions.code",
            "reason_code",
            "module_operations",
            "correlation_id",
            "requested_by",
        ] {
            assert!(
                !helper_body.contains(forbidden_fragment),
                "{} must not contain forbidden helper-level fragment `{forbidden_fragment}`",
                case.signature
            );
        }
    }
}

#[test]
fn module_graphql_mutation_constants_have_stable_operation_shapes() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    let cases = [
        (
            "pub const INSTALL_MODULE_MUTATION: &str = \"",
            [
                "mutation InstallModule(",
                "installModule(",
                "slug: $slug",
                "version: $version",
                "manifestRef",
                "manifestHash",
                "manifestRevision",
            ],
            ["__typename", "toggleModule(", "moduleSlug: $moduleSlug"],
        ),
        (
            "pub const UNINSTALL_MODULE_MUTATION: &str = \"",
            [
                "mutation UninstallModule(",
                "uninstallModule(",
                "slug: $slug",
                "manifestRef",
                "manifestHash",
                "manifestRevision",
            ],
            ["__typename", "toggleModule(", "version: $version"],
        ),
        (
            "pub const UPGRADE_MODULE_MUTATION: &str = \"",
            [
                "mutation UpgradeModule(",
                "upgradeModule(",
                "slug: $slug",
                "version: $version",
                "manifestRef",
                "manifestHash",
                "manifestRevision",
            ],
            ["__typename", "toggleModule(", "moduleSlug: $moduleSlug"],
        ),
        (
            "pub const TOGGLE_MODULE_MUTATION: &str = \"",
            [
                "mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!)",
                "toggleModule(moduleSlug: $moduleSlug, enabled: $enabled)",
                "moduleSlug",
                "enabled",
                "settings",
            ],
            [
                "__typename",
                "$module_slug",
                "module_slug:",
                "toggleModule(moduleSlug: $module_slug",
            ],
        ),
    ];

    for (declaration, required_fragments, forbidden_fragments) in cases {
        let mutation = extract_const_string_literal(&content, declaration)
            .unwrap_or_else(|| panic!("mutation declaration not found: {declaration}"));

        for required in required_fragments {
            assert!(
                mutation.contains(required),
                "mutation shape drifted: missing `{required}` for declaration `{declaration}`"
            );
        }
        for forbidden in forbidden_fragments {
            assert!(
                !mutation.contains(forbidden),
                "mutation contract must not contain `{forbidden}` for declaration `{declaration}`"
            );
        }
    }
}


fn assert_graphql_only_helper(
    content: &str,
    signature: &str,
    mutation_name: &str,
    variables_literal: &str,
    forwarded_fields: &[&str],
    return_expr: &str,
) {
    let helper_body = extract_function_block(content, signature)
        .unwrap_or_else(|| panic!("helper signature not found: {signature}"));

    assert!(
        helper_body.contains(mutation_name),
        "expected helper {signature} to call canonical mutation {mutation_name}"
    );
    assert!(
        helper_body.contains("request("),
        "expected helper {signature} to call GraphQL request path"
    );
    assert!(
        helper_body.contains(variables_literal),
        "expected helper {signature} to construct typed GraphQL variables payload"
    );
    assert!(
        helper_body.contains(return_expr),
        "expected helper {signature} to return GraphQL payload directly"
    );
    for field in forwarded_fields {
        assert!(
            helper_body.contains(field),
            "expected helper {signature} to forward field `{field}` into typed GraphQL payload"
        );
    }
    assert!(
        !helper_body.contains("combine_native_and_graphql_error"),
        "helper {signature} must not compose native/graphql fallback errors"
    );
}

#[test]
fn module_composition_helper_signatures_are_unique() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_path = crate_root.join("src/features/modules/api.rs");
    let content = fs::read_to_string(&api_path).expect("read api.rs");

    for signature in [
        "pub async fn install_module(",
        "pub async fn uninstall_module(",
        "pub async fn upgrade_module(",
        "pub async fn toggle_module(",
    ] {
        let occurrences = content.matches(signature).count();
        assert_eq!(
            occurrences, 1,
            "Expected exactly one `{signature}` helper signature, found {occurrences}"
        );
    }
}

fn extract_function_block<'a>(content: &'a str, signature: &str) -> Option<&'a str> {
    let start = content.find(signature)?;
    let rest = &content[start..];
    let open_rel = rest.find('{')?;
    let mut depth = 0usize;
    let mut end_rel = None;

    for (idx, ch) in rest.char_indices().skip(open_rel) {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    end_rel = Some(idx + ch.len_utf8());
                    break;
                }
            }
            _ => {}
        }
    }

    end_rel.map(|end| &rest[..end])
}

fn extract_const_string_literal<'a>(content: &'a str, declaration: &str) -> Option<&'a str> {
    let start = content.find(declaration)?;
    let rest = &content[start + declaration.len()..];
    let end = rest.find("\";")?;
    Some(&rest[..end])
}

#[test]
fn extract_function_block_handles_nested_braces() {
    let source = r#"
pub async fn toggle_module() -> Result<(), ()> {
    if true {
        let nested = || {
            let map = std::collections::BTreeMap::<String, String>::new();
            map
        };
        let _ = nested();
    }
    Ok(())
}

pub async fn other_helper() {}
"#;

    let extracted = extract_function_block(source, "pub async fn toggle_module()")
        .expect("toggle_module function should be extracted");
    assert!(extracted.contains("BTreeMap::<String, String>::new()"));
    assert!(extracted.trim_end().ends_with('}'));
    assert!(!extracted.contains("pub async fn other_helper()"));
}

#[test]
fn extract_function_block_returns_none_when_signature_missing() {
    let source = "pub async fn other_helper() {}";
    assert!(extract_function_block(source, "pub async fn toggle_module(").is_none());
}

#[test]
fn extract_function_block_returns_none_when_braces_are_unbalanced() {
    let source = r#"
pub async fn toggle_module() {
    if true {
        let _x = 1;
}
"#;
    assert!(extract_function_block(source, "pub async fn toggle_module()").is_none());
}

#[test]
fn extract_function_block_returns_none_when_body_brace_missing() {
    let source = "pub async fn toggle_module() -> Result<(), ()>";
    assert!(extract_function_block(source, "pub async fn toggle_module()").is_none());
}
