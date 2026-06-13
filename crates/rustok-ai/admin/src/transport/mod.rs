//! AI admin transport facade.
//!
//! The Leptos adapter calls this module instead of raw server-function
//! endpoints. Native server functions currently live in `native_server_adapter`;
//! GraphQL/headless parity remains outside this Leptos-only facade until the
//! next adapter split.

pub mod native_server_adapter;

pub use native_server_adapter::{
    create_provider, create_task_profile, create_tool_profile, deactivate_provider,
    fetch_bootstrap, fetch_session, resume_approval, run_task_job, send_message, start_session,
    test_provider, update_provider, update_task_profile, update_tool_profile, ApiError,
};
