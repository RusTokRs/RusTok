#[test]
fn event_bus_runtime_is_atomically_owned_and_restartable() {
    let source = include_str!("../src/services/event_bus.rs");

    let lock_registration = source
        .find("shared_insert_if_absent(EventBusStartLock::default())")
        .expect("event bus startup must register one context-owned start lock");
    let lock_acquisition = source
        .find("let _start_guard = start_lock")
        .expect("event bus startup must acquire the start lock");
    let bus_lookup = source
        .find("shared_get::<SharedEventBus>()")
        .expect("event bus startup must reuse the shared bus");
    assert!(lock_registration < lock_acquisition);
    assert!(lock_acquisition < bus_lookup);

    assert!(source.contains("struct AbortOnDropEventForwarderTask"));
    assert!(source.contains("impl Drop for AbortOnDropEventForwarderTask"));
    assert!(source.contains("self.task.abort();"));
    assert!(source.contains("pub fn is_running(&self) -> bool"));
    assert!(source.contains("Event forwarder stopped; replacing supervised runtime"));

    assert!(source.contains("async fn supervise_event_forwarder"));
    assert!(source.contains(".catch_unwind()"));
    assert!(source.contains("Event forwarder panicked; restarting"));
    assert!(source.contains("Event forwarder exited unexpectedly; restarting"));
    assert!(source.contains("EVENT_FORWARDER_RESTART_DELAY"));

    assert!(!source.contains("EventForwarderHandle {\n        _handle:"));
}

#[test]
fn terminal_event_forwarder_is_critical_in_runtime_guardrails() {
    let source = include_str!("../src/services/runtime_guardrails.rs");
    let forwarder = source
        .find("ctx.shared_get::<EventForwarderHandle>()")
        .expect("runtime guardrails must observe the event forwarder handle");
    let rbac = source
        .find("ctx.shared_get::<RbacCacheInvalidationListenerHandle>()")
        .expect("existing critical worker guard must remain present");

    assert!(forwarder < rbac);
    assert!(source.contains("event bus transport forwarder"));
    assert!(source.contains("ctx.shared_get::<EventForwarderHandle>()\n            .map(|handle| handle.is_running())"));
    assert!(source.contains("RuntimeGuardrailStatus::Critical"));
}
