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

verifier = "scripts/verify/verify-commerce-return-completion-order-owner-port-cutover.mjs"
replace_once(
    verifier,
    '''const returns = read("crates/rustok-commerce/src/controllers/admin/returns.rs");\n''',
    '''const returns = read("crates/rustok-commerce/src/controllers/admin/returns.rs");\nconst changes = read("crates/rustok-commerce/src/controllers/admin/changes.rs");\n''',
)
replace_once(
    verifier,
    '''requireText(returns, "runtime.order_post_order_command_port()", "REST host-selected Order command port");\n''',
    '''requireText(returns, "runtime.order_post_order_command_port()", "REST host-selected Order command port");\nrequireText(changes, "PostOrderOrchestrationError::OrderPort(source)", "admin order-change exhaustive Order port error mapping");\nrequireText(changes, "admin_order_change_port_error_policy(source)", "admin order-change bounded Order port error policy");\n''',
)
