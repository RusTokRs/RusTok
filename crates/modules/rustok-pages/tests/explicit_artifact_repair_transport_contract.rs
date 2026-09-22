use async_graphql::{EmptySubscription, Schema};
use rustok_pages::{PagesMutation, PagesQuery};
use serde_json::Value;

fn graphql_type_block<'a>(sdl: &'a str, type_name: &str) -> &'a str {
    let marker = format!("type {type_name} {{");
    let start = sdl
        .find(&marker)
        .unwrap_or_else(|| panic!("missing GraphQL type {type_name}"));
    let tail = &sdl[start..];
    let end = tail
        .find("\n}")
        .unwrap_or_else(|| panic!("unterminated GraphQL type {type_name}"));
    &tail[..end + 2]
}

fn schema_properties<'a>(document: &'a Value, name: &str) -> &'a serde_json::Map<String, Value> {
    document["components"]["schemas"][name]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("missing OpenAPI schema properties for {name}"))
}

#[test]
fn repair_graphql_schema_mounts_separate_bounded_mutations() {
    let schema = Schema::build(
        PagesQuery::default(),
        PagesMutation::default(),
        EmptySubscription,
    )
    .finish();
    let sdl = schema.sdl();

    assert!(sdl.contains("rebuildPageArtifact("));
    assert!(sdl.contains("activateRebuiltPageArtifact("));
    assert!(sdl.contains("input RebuildGqlPageArtifactInput"));
    assert!(sdl.contains("input ActivateGqlRebuiltPageArtifactInput"));

    let rebuild = graphql_type_block(&sdl, "GqlRebuildPageArtifactResult");
    for required in [
        "operationId:",
        "pageId:",
        "locale:",
        "sourceArtifactId:",
        "rebuiltArtifactId:",
        "artifactHash:",
        "materializationHash:",
        "replayed:",
        "rebuiltAt:",
    ] {
        assert!(
            rebuild.contains(required),
            "missing rebuild result field {required}"
        );
    }
    for forbidden in [
        "sourceId:",
        "sourcePublishOperationId:",
        "artifactInstanceKey:",
        "idempotencyKey:",
        "runtime:",
        "materializationIdentity:",
        "runtimeSnapshots:",
    ] {
        assert!(
            !rebuild.contains(forbidden),
            "rebuild result leaked internal field {forbidden}"
        );
    }

    let activation = graphql_type_block(&sdl, "GqlActivateRebuiltPageArtifactResult");
    for required in [
        "operationId:",
        "pageId:",
        "version:",
        "locale:",
        "rebuildOperationId:",
        "previousArtifactId:",
        "replacementArtifactId:",
        "replacementArtifactHash:",
        "replacementMaterializationHash:",
        "replayed:",
        "replacedAt:",
    ] {
        assert!(
            activation.contains(required),
            "missing activation result field {required}"
        );
    }
    for forbidden in [
        "sourceId:",
        "sourcePublishOperationId:",
        "artifactInstanceKey:",
        "idempotencyKey:",
        "runtime:",
        "materializationIdentity:",
        "runtimeSnapshots:",
    ] {
        assert!(
            !activation.contains(forbidden),
            "activation result leaked internal field {forbidden}"
        );
    }
}

#[test]
fn repair_openapi_registers_routes_and_bounded_result_schemas() {
    let document = serde_json::to_value(rustok_pages::openapi::openapi_document())
        .expect("Pages OpenAPI document must serialize");

    for route in [
        "/api/admin/pages/{id}/artifacts/rebuild",
        "/api/admin/pages/{id}/artifacts/activate",
    ] {
        assert!(
            document["paths"][route]["post"].is_object(),
            "missing POST {route}"
        );
    }

    let rebuild = schema_properties(&document, "RebuildPageArtifactTransportResult");
    for required in [
        "operation_id",
        "page_id",
        "locale",
        "source_artifact_id",
        "rebuilt_artifact_id",
        "artifact_hash",
        "materialization_hash",
        "replayed",
        "rebuilt_at",
    ] {
        assert!(
            rebuild.contains_key(required),
            "missing rebuild OpenAPI field {required}"
        );
    }
    for forbidden in [
        "source_id",
        "source_publish_operation_id",
        "artifact_instance_key",
        "idempotency_key",
        "runtime",
        "materialization_identity",
        "runtime_snapshots",
    ] {
        assert!(
            !rebuild.contains_key(forbidden),
            "rebuild OpenAPI result leaked {forbidden}"
        );
    }

    let activation = schema_properties(&document, "ActivateRebuiltPageArtifactTransportResult");
    for required in [
        "operation_id",
        "page_id",
        "version",
        "locale",
        "rebuild_operation_id",
        "previous_artifact_id",
        "replacement_artifact_id",
        "replacement_artifact_hash",
        "replacement_materialization_hash",
        "replayed",
        "replaced_at",
    ] {
        assert!(
            activation.contains_key(required),
            "missing activation OpenAPI field {required}"
        );
    }
    for forbidden in [
        "source_id",
        "source_publish_operation_id",
        "artifact_instance_key",
        "idempotency_key",
        "runtime",
        "materialization_identity",
        "runtime_snapshots",
    ] {
        assert!(
            !activation.contains_key(forbidden),
            "activation OpenAPI result leaked {forbidden}"
        );
    }

    let schemas = &document["components"]["schemas"];
    assert!(schemas["RebuildPageArtifactInput"].is_object());
    assert!(schemas["ReplacePageArtifactBindingInput"].is_object());
    assert!(schemas["RebuildPageArtifactTransportResult"].is_object());
    assert!(schemas["ActivateRebuiltPageArtifactTransportResult"].is_object());
}
