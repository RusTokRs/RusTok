use rustok_core::module::{ModuleKind, RusToKModule};

use crate::CommentsModule;

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public surface",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn module_contract_metadata_matches() {
    let module = CommentsModule::new();
    assert_eq!(module.slug(), "comments");
    assert_eq!(module.name(), "Comments");
    assert_eq!(module.kind(), ModuleKind::Optional);
}

#[tokio::test]
async fn comments_service_and_adapters_debug_formatting() {
    use sea_orm::Database;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let service = crate::CommentsService::new(db.clone());
    let debug_str = format!("{service:?}");
    assert!(debug_str.contains("CommentsService"));
    assert!(debug_str.contains("has_event_bus: false"));

    let handler = crate::target_lifecycle::CommentTargetDeletionHandler::new(db.clone());
    let handler_debug = format!("{handler:?}");
    assert!(handler_debug.contains("CommentTargetDeletionHandler"));

    struct DummyTransport;
    #[async_trait::async_trait]
    impl crate::remote::CommentsThreadTransport for DummyTransport {
        async fn execute(
            &self,
            _req: crate::remote::CommentsThreadRequest,
        ) -> Result<crate::remote::CommentsThreadResponse, rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable("dummy", "dummy"))
        }
    }
    let remote = crate::remote::RemoteCommentsThreadPort::new(std::sync::Arc::new(DummyTransport));
    let remote_debug = format!("{remote:?}");
    assert!(remote_debug.contains("RemoteCommentsThreadPort"));
}

#[cfg(feature = "tcp-transport")]
#[test]
fn comments_tcp_server_adapter_debug_formatting() {
    use std::sync::Arc;
    struct DummyPort;
    #[async_trait::async_trait]
    impl crate::CommentsThreadPort for DummyPort {
        async fn create_comment(
            &self,
            _c: rustok_api::PortContext,
            _r: crate::CreateCommentInput,
        ) -> Result<crate::CommentRecord, rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable("dummy", "dummy"))
        }
        async fn get_comment(
            &self,
            _c: rustok_api::PortContext,
            _id: uuid::Uuid,
            _f: Option<String>,
        ) -> Result<crate::CommentRecord, rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable("dummy", "dummy"))
        }
        async fn list_comments_for_target(
            &self,
            _c: rustok_api::PortContext,
            _t: String,
            _id: uuid::Uuid,
            _f: crate::ListCommentsFilter,
            _l: Option<String>,
        ) -> Result<(Vec<crate::CommentListItem>, u64), rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable("dummy", "dummy"))
        }
        async fn list_public_comments_for_target(
            &self,
            _c: rustok_api::PortContext,
            _t: String,
            _id: uuid::Uuid,
            _f: crate::ListCommentsFilter,
            _l: Option<String>,
        ) -> Result<(Vec<crate::CommentListItem>, u64), rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable("dummy", "dummy"))
        }
        async fn update_comment(
            &self,
            _c: rustok_api::PortContext,
            _id: uuid::Uuid,
            _r: crate::UpdateCommentInput,
        ) -> Result<crate::CommentRecord, rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable("dummy", "dummy"))
        }
        async fn set_comment_status(
            &self,
            _c: rustok_api::PortContext,
            _id: uuid::Uuid,
            _r: crate::SetCommentStatusRequest,
        ) -> Result<crate::CommentRecord, rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable("dummy", "dummy"))
        }
        async fn delete_comment(
            &self,
            _c: rustok_api::PortContext,
            _id: uuid::Uuid,
        ) -> Result<(), rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable("dummy", "dummy"))
        }
    }
    struct DummyResolver;
    #[async_trait::async_trait]
    impl crate::tcp_server::CommentsTcpAuthorityResolver for DummyResolver {
        async fn authorize(
            &self,
            _p: std::net::SocketAddr,
            _op: crate::tcp_server::CommentsTcpOperation,
            _cr: Option<&crate::tcp_auth::CommentsTcpCredential>,
            _rq: &crate::remote::CommentsThreadRequest,
        ) -> Result<crate::tcp_server::TrustedCommentsTcpAuthority, rustok_api::PortError> {
            Err(rustok_api::PortError::forbidden("dummy", "dummy"))
        }
    }
    let adapter = crate::tcp_server::TcpJsonCommentsServerAdapter::new(
        Arc::new(DummyPort),
        Arc::new(DummyResolver),
    );
    let debug = format!("{adapter:?}");
    assert!(debug.contains("TcpJsonCommentsServerAdapter"));
    assert!(debug.contains("max_frame_bytes"));
}
