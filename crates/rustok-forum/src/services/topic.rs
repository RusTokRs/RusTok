// The established topic implementation and the FORUM-12B2 relation hook share
// one module so the hook can reuse private transaction helpers without widening
// the crate API.
include!("topic_core.rs");
include!("topic_relation_integration.rs");
