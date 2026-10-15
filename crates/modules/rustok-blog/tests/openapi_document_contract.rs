//! The published Blog OpenAPI document must describe every route the module serves.
//!
//! `BlogApiDoc` is merged into the server-wide document
//! (`apps/server/src/controllers/swagger.rs`), so a handler that carries a
//! `#[utoipa::path]` attribute but is absent from the registration list is invisible to
//! every generated client. The 2026-10-15 audit (M-6) found three such handlers —
//! `preview_post`, `pin_post` and `unpin_post` — and asked for a gate that keeps the
//! document and the router together. Because the text verifiers can only read the
//! registration list, this gate is compiled and executed with `cargo test -p rustok-blog`
//! and checks the document utoipa actually generates.

use rustok_blog::openapi::openapi_document;

/// The module router, the registration list, and the three controller files the router
/// references. `comment_assets` is feature-gated and declares no `#[utoipa::path]`, so it
/// is deliberately not part of this contract.
const CONTROLLERS_MOD: &str = include_str!("../src/controllers/mod.rs");
const OPENAPI_SOURCE: &str = include_str!("../src/controllers/openapi.rs");
const CONTROLLER_SOURCES: [(&str, &str); 3] = [
    ("posts.rs", include_str!("../src/controllers/posts.rs")),
    (
        "categories.rs",
        include_str!("../src/controllers/categories.rs"),
    ),
    (
        "comments.rs",
        include_str!("../src/controllers/comments.rs"),
    ),
];

/// The number of paths `controllers::axum_router` registers today. A new route must be
/// added to the document and to the controller set below in the same change.
const ROUTER_PATH_COUNT: usize = 13;
/// The number of handlers that declare a `#[utoipa::path]` in the controller set.
const DOCUMENTED_HANDLER_COUNT: usize = 19;

/// Path literals of the routes `controllers::axum_router` registers. The feature-gated
/// asset router is merged in, not registered with `.route`, so it cannot appear here.
fn router_paths() -> Vec<String> {
    let mut paths = Vec::new();
    for chunk in CONTROLLERS_MOD.split(".route(").skip(1) {
        let Some(quote) = chunk.find('"') else {
            continue;
        };
        let rest = &chunk[quote + 1..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        paths.push(rest[..end].to_string());
    }
    paths
}

/// `(handler, path)` pairs declared by `#[utoipa::path]` attributes. Each attribute is
/// read up to the handler signature it documents, so no arbitrary window is sliced out of
/// the source.
fn declared_handlers(source: &str) -> Vec<(String, String)> {
    let mut handlers = Vec::new();
    for (index, _) in source.match_indices("#[utoipa::path(") {
        let rest = &source[index..];
        let Some(signature) = rest.find("pub async fn ") else {
            continue;
        };
        let attribute = &rest[..signature];
        let Some(path) = attribute
            .split_once("path = \"")
            .and_then(|(_, tail)| tail.split_once('"'))
            .map(|(path, _)| path.to_string())
        else {
            continue;
        };
        let handler = rest[signature + "pub async fn ".len()..]
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .next()
            .unwrap_or_default();
        assert!(
            !handler.is_empty(),
            "could not read the handler name that follows a #[utoipa::path] attribute"
        );
        handlers.push((handler.to_string(), path));
    }
    handlers
}

#[test]
fn document_lists_every_route_the_router_registers() {
    let document = openapi_document();
    let paths = router_paths();
    assert_eq!(
        paths.len(),
        ROUTER_PATH_COUNT,
        "the Blog router should register {ROUTER_PATH_COUNT} paths, found {paths:?}"
    );
    for path in paths {
        assert!(
            document.paths.paths.contains_key(path.as_str()),
            "Blog OpenAPI document is missing router path {path}"
        );
    }
}

#[test]
fn document_lists_every_handler_that_declares_a_utoipa_path() {
    let document = openapi_document();
    let mut declared = 0;
    for (file, source) in CONTROLLER_SOURCES {
        let handlers = declared_handlers(source);
        assert!(
            !handlers.is_empty(),
            "{file} should declare #[utoipa::path] handlers"
        );
        for (handler, path) in handlers {
            declared += 1;
            assert!(
                OPENAPI_SOURCE.contains(handler.as_str()),
                "{file}:{handler} is not registered in BlogApiDoc"
            );
            assert!(
                document.paths.paths.contains_key(path.as_str()),
                "Blog OpenAPI document is missing {handler}'s path {path}"
            );
        }
    }
    assert_eq!(
        declared, DOCUMENTED_HANDLER_COUNT,
        "the Blog controllers should declare {DOCUMENTED_HANDLER_COUNT} documented handlers, found {declared}"
    );
}
