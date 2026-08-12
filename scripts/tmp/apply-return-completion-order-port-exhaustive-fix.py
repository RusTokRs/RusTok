from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one match, found {count}: {old[:100]!r}")
    p.write_text(text.replace(old, new, 1))


changes = "crates/rustok-commerce/src/controllers/admin/changes.rs"
replace_once(
    changes,
    '''        PostOrderOrchestrationError::Payment(source) => {\n''',
    '''        PostOrderOrchestrationError::OrderPort(source) => {\n            let (status, code, message, error_kind) =\n                admin_order_change_port_error_policy(source);\n            (status, code, message, error_kind, "rustok_order")\n        }\n        PostOrderOrchestrationError::Payment(source) => {\n''',
)

core = "crates/rustok-commerce/src/services/return_completion_orchestration.rs"
replace_once(
    core,
    '''    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {\n        let order_reads = in_process_order_read_port(db.clone(), event_bus.clone());\n        let order_commands = in_process_order_post_order_command_port(db.clone(), event_bus);\n        Self::from_order_ports(db, order_reads, order_commands)\n    }\n\n''',
    "",
)
replace_once(
    core,
    '''    ReadOrderReturnProjectionRequest, in_process_order_post_order_command_port,\n    in_process_order_read_port,\n};\nuse rustok_order::error::OrderError;\nuse rustok_outbox::TransactionalEventBus;\n''',
    '''    ReadOrderReturnProjectionRequest,\n};\nuse rustok_order::error::OrderError;\n''',
)

runtime = "crates/rustok-commerce/src/graphql_runtime.rs"
replace_once(
    runtime,
    '''        None => crate::ReturnCompletionOrchestrationService::new(db, event_bus),\n''',
    '''        None => {\n            let order_reads = in_process_order_read_port(db.clone(), event_bus.clone());\n            let order_commands =\n                OrderPostOrderCommandRuntime::in_process(db.clone(), event_bus).command_port();\n            crate::ReturnCompletionOrchestrationService::from_order_ports(\n                db,\n                order_reads,\n                order_commands,\n            )\n        }\n''',
)

verifier = "scripts/verify/verify-commerce-return-completion-order-owner-port-cutover.mjs"
replace_once(
    verifier,
    '''const returns = read("crates/rustok-commerce/src/controllers/admin/returns.rs");\n''',
    '''const returns = read("crates/rustok-commerce/src/controllers/admin/returns.rs");\nconst changes = read("crates/rustok-commerce/src/controllers/admin/changes.rs");\n''',
)
replace_once(
    verifier,
    '''forbidText(core, "OrderService::new", "core direct Order service construction");\n''',
    '''forbidText(core, "OrderService::new", "core direct Order service construction");\nforbidText(core, "pub fn new(db: DatabaseConnection", "legacy in-process return completion constructor");\n''',
)
replace_once(
    verifier,
    '''requireText(returns, "runtime.order_post_order_command_port()", "REST host-selected Order command port");\n''',
    '''requireText(returns, "runtime.order_post_order_command_port()", "REST host-selected Order command port");\nrequireText(changes, "PostOrderOrchestrationError::OrderPort(source)", "admin order-change exhaustive Order port error mapping");\nrequireText(changes, "admin_order_change_port_error_policy(source)", "admin order-change bounded Order port error policy");\n''',
)
replace_once(
    verifier,
    '''requireText(graphqlRuntime, "runtime.order_post_order_command_runtime().command_port()", "GraphQL Order command runtime");\n''',
    '''requireText(graphqlRuntime, "runtime.order_post_order_command_runtime().command_port()", "GraphQL Order command runtime");\nrequireText(graphqlRuntime, "in_process_order_read_port(db.clone(), event_bus.clone())", "GraphQL embedded Order read fallback");\nrequireText(graphqlRuntime, "OrderPostOrderCommandRuntime::in_process(db.clone(), event_bus)", "GraphQL embedded Order command fallback");\n''',
)

guard = "apps/server/tests/commerce_return_completion_transport_guard.rs"
replace_once(
    guard,
    '''        rest.contains("ReturnCompletionOrchestrationService::new("),\n        "REST return completion must use the commerce orchestration boundary"\n''',
    '''        rest.contains("ReturnCompletionOrchestrationService::from_order_ports("),\n        "REST return completion must compose the commerce orchestration from owner ports"\n''',
)
replace_once(
    guard,
    '''        ".complete_return(tenant_id, return_id, owner_input)",\n''',
    '''        "CompleteOrderReturnRequest {",\n''',
)
replace_once(
    guard,
    '''        .find(".complete_return(tenant_id, return_id, owner_input)")\n''',
    '''        .find("CompleteOrderReturnRequest {")\n''',
)
