const ADAPTER: &str = include_str!("../src/transport/native_server_adapter.rs");
const SNAPSHOT: &str = include_str!("../src/transport/native_server_adapter/snapshot.rs");

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let (_, rest) = source.split_once(start).expect(start);
    rest.split_once(end).map_or(rest, |(body, _)| body)
}

#[test]
fn native_index_endpoints_use_the_same_request_tenant_boundary() {
    let auth = section(ADAPTER, "async fn authenticated_context", "fn host_context");
    for marker in [
        "extract::<AuthContext>()",
        "extract::<TenantContext>()",
        "auth.tenant_id.is_nil()",
        "tenant.id.is_nil()",
        "auth.user_id.is_nil()",
        "auth.tenant_id != tenant.id",
    ] {
        assert!(auth.contains(marker), "missing tenant guard: {marker}");
    }

    let bootstrap = section(ADAPTER, "pub async fn fetch_bootstrap_native", "#[server");
    let tenant_guard = bootstrap.find("authenticated_context().await?").unwrap();
    let read_permission = bootstrap.find("authorize_bootstrap(&auth)?").unwrap();
    let read_snapshot = bootstrap
        .find("snapshot::read(&runtime, tenant, can_manage)")
        .unwrap();
    assert!(tenant_guard < read_permission && read_permission < read_snapshot);

    let operator = section(ADAPTER, "async fn operator_context", "fn operator_error");
    assert!(
        operator.find("authenticated_context().await?").unwrap()
            < operator
                .find("authorize_operator(&auth, &tenant)?")
                .unwrap()
    );
    let authority = section(ADAPTER, "fn authorize_operator", "fn replay_runtime");
    assert!(authority.contains(
        "auth.tenant_id,
        auth.user_id,
        &auth.permissions,"
    ));
    for endpoint in [
        "trigger_replay_native",
        "cancel_job_native",
        "retry_job_native",
    ] {
        assert!(
            section(ADAPTER, &format!("pub async fn {endpoint}"), "#[server")
                .contains("operator_context().await?")
        );
    }
}

#[test]
fn native_job_commands_and_snapshot_queries_keep_the_tenant_bound() {
    let job_id_parser = section(ADAPTER, "fn exact_job_id", "#[server");
    assert!(job_id_parser.contains("!id.is_nil()"));
    assert_eq!(ADAPTER.matches("exact_job_id(&input.job_id)?").count(), 2);
    assert!(ADAPTER.contains("cancel_operator_replay(authority, job_id)"));
    assert!(ADAPTER.contains("requeue_operator_replay(authority, job_id, reason)"));
    assert!(ADAPTER.contains("requeue_operator_reconciliation(authority, job_id, reason)"));
    for table in [
        "index_schemas",
        "index_inbox",
        "index_jobs",
        "index_job_recovery_audits",
    ] {
        assert!(SNAPSHOT.contains(&format!("FROM {table} WHERE tenant_id = $1")));
    }
}
