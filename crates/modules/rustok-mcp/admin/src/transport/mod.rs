//! MCP admin transport facade.
//!
//! Leptos UI calls this facade. Native server-function endpoints are the
//! default Leptos data layer; GraphQL operation documents remain available for
//! host-owned GraphQL clients.

pub mod graphql_adapter;
pub mod native_server_adapter;

pub use native_server_adapter::{
    ApiError, apply_scaffold_draft, create_client, deactivate_client, fetch_audit_events,
    fetch_client_details, fetch_clients, fetch_scaffold_drafts, revoke_token, rotate_token,
    stage_scaffold_draft, update_policy,
};
