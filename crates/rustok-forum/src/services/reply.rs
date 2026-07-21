// The established reply implementation and the FORUM-12B2 relation hook share
// one module so the hook can reuse private transaction helpers without widening
// the crate API.
include!("reply_core.rs");
include!("reply_relation_integration.rs");
