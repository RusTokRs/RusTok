use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_source(path: impl AsRef<Path>) -> String {
    let path = manifest_dir().join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "failed to read comments admin boundary source {}: {error}",
            path.display()
        )
    })
}

#[test]
fn comments_admin_uses_private_transport_facade_without_pre_ffa_api() {
    let lib = read_source("src/lib.rs");
    let transport = read_source("src/transport/mod.rs");
    let native_adapter = read_source("src/transport/native_server_adapter.rs");
    let ui = read_source("src/ui/leptos.rs");

    assert!(
        !manifest_dir().join("src/api.rs").exists(),
        "src/api.rs must stay removed after comments admin FFA transport split"
    );
    assert!(
        lib.contains("mod transport;"),
        "crate root must wire the private comments admin transport facade"
    );
    assert!(
        !lib.contains("mod api;"),
        "crate root must not wire the pre-FFA api facade"
    );
    assert!(
        lib.contains("pub use ui::leptos::CommentsAdmin;"),
        "crate root should keep exporting only the Leptos admin entry point"
    );

    for marker in [
        "CommentsAdminTransportError",
        "CommentThreadsPayload",
        "ACTIVE_TRANSPORT_PATH",
        "native_server_adapter::fetch_threads",
        "native_server_adapter::fetch_thread_detail",
        "native_server_adapter::set_thread_status",
        "native_server_adapter::set_comment_status",
    ] {
        assert!(
            transport.contains(marker),
            "transport facade must keep comments admin marker `{marker}`"
        );
    }

    for marker in [
        "comments_threads_native",
        "comments_thread_detail_native",
        "comments_set_thread_status_native",
        "comments_set_comment_status_native",
        "CommentsService::new",
    ] {
        assert!(
            native_adapter.contains(marker),
            "native adapter must own comments server-function marker `{marker}`"
        );
    }

    assert!(
        ui.contains("transport::fetch_threads"),
        "UI must call the comments transport facade for thread lists"
    );
    assert!(
        ui.contains("transport::fetch_thread_detail"),
        "UI must call the comments transport facade for thread details"
    );
    assert!(
        ui.contains("transport::set_thread_status"),
        "UI must call the comments transport facade for thread status mutations"
    );
    assert!(
        ui.contains("transport::set_comment_status"),
        "UI must call the comments transport facade for comment status mutations"
    );
    assert!(
        !ui.contains("crate::api"),
        "UI must not call the removed comments api facade"
    );
}

#[test]
fn comments_admin_native_only_exception_is_documented() {
    let admin_readme = read_source("README.md");
    let module_docs = read_source("../docs/README.md");

    for marker in [
        "Does not introduce a new GraphQL or REST transport just for parity",
        "has no legacy transport surface",
    ] {
        assert!(
            admin_readme.contains(marker),
            "admin README must document native-only transport marker `{marker}`"
        );
    }

    for marker in [
        "Отдельный GraphQL/REST fallback для этого UI не добавляется",
        "зафиксированное исключение из общего dual-path правила",
    ] {
        assert!(
            module_docs.contains(marker),
            "module docs must document native-only transport marker `{marker}`"
        );
    }
}
