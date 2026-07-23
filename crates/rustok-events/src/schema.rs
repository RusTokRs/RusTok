use schemars::schema_for;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct FieldSchema {
    pub name: &'static str,
    pub data_type: &'static str,
    pub optional: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct EventSchema {
    pub event_type: &'static str,
    pub version: u16,
    pub description: &'static str,
    pub fields: &'static [FieldSchema],
}

impl EventSchema {
    pub fn to_json_schema(&self) -> serde_json::Value {
        let properties: serde_json::Map<String, serde_json::Value> = self
            .fields
            .iter()
            .map(|field| {
                let mut schema = field_json_schema(field.data_type);
                if field.optional {
                    schema = serde_json::json!({
                        "anyOf": [schema, { "type": "null" }],
                    });
                }
                (field.name.to_string(), schema)
            })
            .collect();

        let required: Vec<&str> = self
            .fields
            .iter()
            .filter(|field| !field.optional)
            .map(|field| field.name)
            .collect();

        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": self.event_type,
            "type": "object",
            "description": self.description,
            "properties": properties,
            "required": required,
            "version": self.version,
        })
    }
}

fn field_json_schema(data_type: &str) -> serde_json::Value {
    match data_type {
        "uuid" => serde_json::json!({ "type": "string", "format": "uuid" }),
        "int32" => serde_json::json!({
            "type": "integer",
            "minimum": i32::MIN,
            "maximum": i32::MAX,
        }),
        "int64" => serde_json::json!({
            "type": "integer",
            "minimum": i64::MIN,
            "maximum": i64::MAX,
        }),
        "uint64" => serde_json::json!({ "type": "integer", "minimum": 0 }),
        "bool" => serde_json::json!({ "type": "boolean" }),
        "string" => serde_json::json!({ "type": "string" }),
        unsupported => serde_json::json!({
            "description": format!("Unsupported RusToK field type: {unsupported}"),
        }),
    }
}

/// Generates the canonical JSON Schema for the serde representation of every
/// established root event variant.
pub fn domain_event_json_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(crate::DomainEvent))
        .expect("schemars output must always serialize to JSON")
}

/// Generates the canonical JSON Schema for the established root envelope.
pub fn event_envelope_json_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(crate::EventEnvelope))
        .expect("schemars output must always serialize to JSON")
}

macro_rules! field {
    ($name:literal, $data_type:literal) => {
        FieldSchema {
            name: $name,
            data_type: $data_type,
            optional: false,
        }
    };
    ($name:literal, $data_type:literal, optional) => {
        FieldSchema {
            name: $name,
            data_type: $data_type,
            optional: true,
        }
    };
}

const NODE_CREATED_FIELDS: &[FieldSchema] = &[
    field!("node_id", "uuid"),
    field!("kind", "string"),
    field!("author_id", "uuid", optional),
];
const NODE_UPDATED_FIELDS: &[FieldSchema] = &[field!("node_id", "uuid"), field!("kind", "string")];
const NODE_TRANSLATION_UPDATED_FIELDS: &[FieldSchema] =
    &[field!("node_id", "uuid"), field!("locale", "string")];
const NODE_PUBLISHED_FIELDS: &[FieldSchema] =
    &[field!("node_id", "uuid"), field!("kind", "string")];
const NODE_UNPUBLISHED_FIELDS: &[FieldSchema] =
    &[field!("node_id", "uuid"), field!("kind", "string")];
const NODE_DELETED_FIELDS: &[FieldSchema] = &[field!("node_id", "uuid"), field!("kind", "string")];
const BODY_UPDATED_FIELDS: &[FieldSchema] =
    &[field!("node_id", "uuid"), field!("locale", "string")];

const CATEGORY_ID_FIELDS: &[FieldSchema] = &[field!("category_id", "uuid")];
const TAG_ID_FIELDS: &[FieldSchema] = &[field!("tag_id", "uuid")];
const TAG_RELATION_FIELDS: &[FieldSchema] = &[
    field!("tag_id", "uuid"),
    field!("target_type", "string"),
    field!("target_id", "uuid"),
];

const MEDIA_UPLOADED_FIELDS: &[FieldSchema] = &[
    field!("media_id", "uuid"),
    field!("mime_type", "string"),
    field!("size", "int64"),
];
const MEDIA_DELETED_FIELDS: &[FieldSchema] = &[field!("media_id", "uuid")];

const USER_REGISTERED_FIELDS: &[FieldSchema] =
    &[field!("user_id", "uuid"), field!("email", "string")];
const USER_LOGGED_IN_FIELDS: &[FieldSchema] = &[field!("user_id", "uuid")];
const USER_UPDATED_FIELDS: &[FieldSchema] = &[field!("user_id", "uuid")];
const PROFILE_UPDATED_FIELDS: &[FieldSchema] = &[
    field!("user_id", "uuid"),
    field!("handle", "string"),
    field!("locale", "string", optional),
];
const USER_DELETED_FIELDS: &[FieldSchema] = &[field!("user_id", "uuid")];

const PRODUCT_ID_FIELDS: &[FieldSchema] = &[field!("product_id", "uuid")];
const VARIANT_FIELDS: &[FieldSchema] =
    &[field!("variant_id", "uuid"), field!("product_id", "uuid")];
const INVENTORY_UPDATED_FIELDS: &[FieldSchema] = &[
    field!("variant_id", "uuid"),
    field!("product_id", "uuid"),
    field!("location_id", "uuid"),
    field!("old_quantity", "int32"),
    field!("new_quantity", "int32"),
];
const INVENTORY_LOW_FIELDS: &[FieldSchema] = &[
    field!("variant_id", "uuid"),
    field!("product_id", "uuid"),
    field!("remaining", "int32"),
    field!("threshold", "int32"),
];
const PRICE_UPDATED_FIELDS: &[FieldSchema] = &[
    field!("variant_id", "uuid"),
    field!("product_id", "uuid"),
    field!("currency", "string"),
    field!("old_amount", "int64", optional),
    field!("new_amount", "int64"),
];
const ORDER_PLACED_FIELDS: &[FieldSchema] = &[
    field!("order_id", "uuid"),
    field!("customer_id", "uuid", optional),
    field!("total", "int64"),
    field!("currency", "string"),
];
const ORDER_STATUS_CHANGED_FIELDS: &[FieldSchema] = &[
    field!("order_id", "uuid"),
    field!("old_status", "string"),
    field!("new_status", "string"),
];
const ORDER_COMPLETED_FIELDS: &[FieldSchema] = &[field!("order_id", "uuid")];
const ORDER_CANCELLED_FIELDS: &[FieldSchema] = &[
    field!("order_id", "uuid"),
    field!("reason", "string", optional),
];

const REINDEX_REQUESTED_FIELDS: &[FieldSchema] = &[
    field!("target_type", "string"),
    field!("target_id", "uuid", optional),
];
const INDEX_UPDATED_FIELDS: &[FieldSchema] =
    &[field!("index_name", "string"), field!("target_id", "uuid")];
const BUILD_REQUESTED_FIELDS: &[FieldSchema] =
    &[field!("build_id", "uuid"), field!("requested_by", "string")];
const BUILD_ROLLED_BACK_FIELDS: &[FieldSchema] = &[
    field!("requested_build_id", "uuid"),
    field!("restored_build_id", "uuid"),
    field!("from_release_id", "string"),
    field!("to_release_id", "string"),
];

const BLOG_POST_CREATED_FIELDS: &[FieldSchema] = &[
    field!("post_id", "uuid"),
    field!("author_id", "uuid", optional),
    field!("locale", "string"),
];
const BLOG_POST_PUBLISHED_FIELDS: &[FieldSchema] = &[
    field!("post_id", "uuid"),
    field!("author_id", "uuid", optional),
];
const BLOG_POST_UNPUBLISHED_FIELDS: &[FieldSchema] = &[field!("post_id", "uuid")];
const BLOG_POST_UPDATED_FIELDS: &[FieldSchema] =
    &[field!("post_id", "uuid"), field!("locale", "string")];
const BLOG_POST_ARCHIVED_FIELDS: &[FieldSchema] = &[
    field!("post_id", "uuid"),
    field!("reason", "string", optional),
];
const BLOG_POST_DELETED_FIELDS: &[FieldSchema] = &[field!("post_id", "uuid")];
const COMMENT_FIELDS: &[FieldSchema] = &[
    field!("comment_id", "uuid"),
    field!("target_type", "string"),
    field!("target_id", "uuid"),
    field!("author_id", "uuid"),
];

const FORUM_TOPIC_CREATED_FIELDS: &[FieldSchema] = &[
    field!("topic_id", "uuid"),
    field!("category_id", "uuid"),
    field!("author_id", "uuid", optional),
    field!("locale", "string"),
];
const FORUM_TOPIC_REPLIED_FIELDS: &[FieldSchema] = &[
    field!("topic_id", "uuid"),
    field!("reply_id", "uuid"),
    field!("author_id", "uuid", optional),
];
const FORUM_TOPIC_STATUS_CHANGED_FIELDS: &[FieldSchema] = &[
    field!("topic_id", "uuid"),
    field!("old_status", "string"),
    field!("new_status", "string"),
    field!("moderator_id", "uuid", optional),
];
const FORUM_TOPIC_PINNED_FIELDS: &[FieldSchema] = &[
    field!("topic_id", "uuid"),
    field!("is_pinned", "bool"),
    field!("moderator_id", "uuid", optional),
];
const FORUM_REPLY_STATUS_CHANGED_FIELDS: &[FieldSchema] = &[
    field!("reply_id", "uuid"),
    field!("topic_id", "uuid"),
    field!("new_status", "string"),
    field!("moderator_id", "uuid", optional),
];

const TOPIC_PROMOTED_TO_POST_FIELDS: &[FieldSchema] = &[
    field!("topic_id", "uuid"),
    field!("post_id", "uuid"),
    field!("moved_comments", "uint64"),
    field!("locale", "string"),
    field!("reason", "string", optional),
];
const POST_DEMOTED_TO_TOPIC_FIELDS: &[FieldSchema] = &[
    field!("post_id", "uuid"),
    field!("topic_id", "uuid"),
    field!("moved_comments", "uint64"),
    field!("locale", "string"),
    field!("reason", "string", optional),
];
const TOPIC_SPLIT_FIELDS: &[FieldSchema] = &[
    field!("source_topic_id", "uuid"),
    field!("target_topic_id", "uuid"),
    field!("moved_comment_ids", "array<uuid>"),
    field!("moved_comments", "uint64"),
    field!("reason", "string", optional),
];
const TOPICS_MERGED_FIELDS: &[FieldSchema] = &[
    field!("target_topic_id", "uuid"),
    field!("moved_comments", "uint64"),
    field!("reason", "string", optional),
];
const CANONICAL_URL_CHANGED_FIELDS: &[FieldSchema] = &[
    field!("target_id", "uuid"),
    field!("target_kind", "string"),
    field!("locale", "string"),
    field!("new_canonical_url", "string"),
    field!("old_urls", "array"),
];
const URL_ALIAS_PURGED_FIELDS: &[FieldSchema] = &[
    field!("target_id", "uuid"),
    field!("target_kind", "string"),
    field!("locale", "string"),
    field!("urls", "array"),
];
const SEO_META_UPSERTED_FIELDS: &[FieldSchema] = &[
    field!("target_kind", "string"),
    field!("target_id", "uuid"),
    field!("locale", "string"),
    field!("source", "string"),
    field!("idempotency_key", "string"),
];
const SEO_REVISION_FIELDS: &[FieldSchema] = &[
    field!("target_kind", "string"),
    field!("target_id", "uuid"),
    field!("revision", "int32"),
    field!("idempotency_key", "string"),
];
const SEO_REDIRECT_UPSERTED_FIELDS: &[FieldSchema] = &[
    field!("redirect_id", "uuid"),
    field!("source_pattern", "string"),
    field!("target_url", "string"),
    field!("status_code", "int32"),
    field!("is_active", "bool"),
    field!("idempotency_key", "string"),
];
const SEO_REDIRECT_DISABLED_FIELDS: &[FieldSchema] = &[
    field!("redirect_id", "uuid"),
    field!("source_pattern", "string"),
    field!("idempotency_key", "string"),
];
const SEO_SITEMAP_GENERATED_FIELDS: &[FieldSchema] = &[
    field!("job_id", "uuid"),
    field!("file_count", "int32"),
    field!("idempotency_key", "string"),
];
const SEO_SITEMAP_SUBMITTED_FIELDS: &[FieldSchema] = &[
    field!("job_id", "uuid"),
    field!("endpoint_count", "int32"),
    field!("success", "bool"),
    field!("error", "string", optional),
    field!("idempotency_key", "string"),
];
const SEO_BULK_COMPLETED_FIELDS: &[FieldSchema] = &[
    field!("job_id", "uuid"),
    field!("target_kind", "string"),
    field!("locale", "string"),
    field!("status", "string"),
    field!("processed_count", "int32"),
    field!("succeeded_count", "int32"),
    field!("failed_count", "int32"),
    field!("idempotency_key", "string"),
];

const TENANT_ID_FIELDS: &[FieldSchema] = &[field!("tenant_id", "uuid")];
const TENANT_MODULE_TOGGLED_FIELDS: &[FieldSchema] = &[
    field!("tenant_id", "uuid"),
    field!("module_slug", "string"),
    field!("enabled", "bool"),
];
const MODULE_ARTIFACT_ADMITTED_FIELDS: &[FieldSchema] = &[
    field!("installation_id", "uuid"),
    field!("artifact_digest", "string"),
    field!("media_type", "string"),
    field!("size_bytes", "uint64"),
];
const MODULE_ARTIFACT_REVERIFIED_FIELDS: &[FieldSchema] = &[
    field!("installation_id", "uuid"),
    field!("status", "string"),
    field!("revision", "uint64"),
];
const ATTRIBUTE_ID_FIELDS: &[FieldSchema] = &[field!("attribute_id", "uuid")];
const ATTRIBUTE_OPTION_FIELDS: &[FieldSchema] =
    &[field!("option_id", "uuid"), field!("attribute_id", "uuid")];
const ATTRIBUTE_SCHEMA_FIELDS: &[FieldSchema] = &[field!("schema_id", "uuid")];
const PRODUCT_PRIMARY_CATEGORY_FIELDS: &[FieldSchema] = &[
    field!("product_id", "uuid"),
    field!("old_category_id", "uuid", optional),
    field!("new_category_id", "uuid", optional),
];
const MODULE_ARTIFACT_ROLLED_BACK_FIELDS: &[FieldSchema] = &[
    field!("installation_id", "uuid"),
    field!("target_installation_id", "uuid"),
];
const MODULE_ARTIFACT_REVISION_FIELDS: &[FieldSchema] = &[
    field!("installation_id", "uuid"),
    field!("revision", "uint64"),
];
const MODULE_ARTIFACT_MIGRATION_CHECKPOINTED_FIELDS: &[FieldSchema] = &[
    field!("installation_id", "uuid"),
    field!("revision", "uint64"),
    field!("has_irreversible_migration", "bool"),
];
const MODULE_ARTIFACT_TENANT_REVISION_FIELDS: &[FieldSchema] = &[
    field!("installation_id", "uuid"),
    field!("tenant_id", "uuid"),
    field!("revision", "uint64"),
];
const MODULE_ARTIFACT_DATA_PURGED_FIELDS: &[FieldSchema] = &[
    field!("tenant_id", "uuid"),
    field!("module_slug", "string"),
    field!("data_contract_revision", "uint64"),
    field!("namespace_revision", "uint64"),
    field!("purged_records", "uint64"),
];
const MODULE_ARTIFACT_DATA_EXPORTED_FIELDS: &[FieldSchema] = &[
    field!("export_id", "uuid"),
    field!("tenant_id", "uuid"),
    field!("module_slug", "string"),
    field!("data_contract_revision", "uint64"),
    field!("namespace_revision", "uint64"),
    field!("exported_records", "uint64"),
];
const MODULE_ARTIFACT_DATA_SNAPSHOT_CREATED_FIELDS: &[FieldSchema] = &[
    field!("snapshot_id", "uuid"),
    field!("tenant_id", "uuid"),
    field!("module_slug", "string"),
    field!("data_contract_revision", "uint64"),
    field!("namespace_revision", "uint64"),
    field!("manifest_digest", "string"),
    field!("structured_records", "uint64"),
    field!("objects", "uint64"),
];
const MODULE_ARTIFACT_DATA_SNAPSHOT_RESTORED_FIELDS: &[FieldSchema] = &[
    field!("snapshot_id", "uuid"),
    field!("tenant_id", "uuid"),
    field!("module_slug", "string"),
    field!("data_contract_revision", "uint64"),
    field!("namespace_revision", "uint64"),
    field!("restored_records", "uint64"),
    field!("restored_objects", "uint64"),
];
const MODULE_ARTIFACT_DATA_SNAPSHOT_RETENTION_FIELDS: &[FieldSchema] = &[
    field!("snapshot_id", "uuid"),
    field!("tenant_id", "uuid"),
    field!("retention_revision", "uint64"),
    field!("retain_until", "string"),
    field!("legal_hold", "bool"),
];
const MODULE_ARTIFACT_DATA_SNAPSHOT_COLLECTED_FIELDS: &[FieldSchema] = &[
    field!("collection_id", "uuid"),
    field!("snapshot_id", "uuid"),
    field!("tenant_id", "uuid"),
    field!("module_slug", "string"),
    field!("data_contract_revision", "uint64"),
    field!("policy_snapshot_id", "string"),
    field!("deleted_objects", "uint64"),
];
const MODULE_ARTIFACT_SECRET_BOUND_FIELDS: &[FieldSchema] = &[
    field!("tenant_id", "uuid"),
    field!("module_slug", "string"),
    field!("data_contract_revision", "uint64"),
    field!("revision", "uint64"),
];
const MODULE_BUILD_QUEUED_FIELDS: &[FieldSchema] = &[
    field!("request_id", "uuid"),
    field!("tenant_id", "uuid"),
    field!("project_id", "string"),
    field!("attempt", "uint64"),
];
const MODULE_BUILD_COMPLETED_FIELDS: &[FieldSchema] = &[
    field!("request_id", "uuid"),
    field!("tenant_id", "uuid"),
    field!("outcome", "string"),
    field!("retryable", "bool"),
];
const MODULE_EFFECTIVE_POLICY_REVISION_CHANGED_FIELDS: &[FieldSchema] = &[
    field!("consumer_key", "string"),
    field!("previous_revision", "string", optional),
    field!("next_revision", "string"),
];
const PLATFORM_SETTINGS_CHANGED_FIELDS: &[FieldSchema] =
    &[field!("category", "string"), field!("changed_by", "uuid")];
const SEARCH_SETTINGS_CHANGED_FIELDS: &[FieldSchema] = &[
    field!("active_engine", "string"),
    field!("fallback_engine", "string"),
    field!("changed_by", "uuid"),
];
const SEARCH_REBUILD_QUEUED_FIELDS: &[FieldSchema] = &[
    field!("target_type", "string"),
    field!("target_id", "uuid", optional),
    field!("queued_by", "uuid"),
];
const MODULE_STATIC_PROMOTION_REQUESTED_FIELDS: &[FieldSchema] = &[
    field!("promotion_id", "uuid"),
    field!("release_id", "string"),
    field!("module_slug", "string"),
    field!("module_version", "string"),
    field!("source_digest", "string"),
];
const MODULE_STATIC_PROMOTION_APPROVED_FIELDS: &[FieldSchema] = &[
    field!("promotion_id", "uuid"),
    field!("release_id", "string"),
    field!("module_slug", "string"),
    field!("module_version", "string"),
    field!("revision", "uint64"),
    field!("policy_revision", "string"),
];
const MODULE_STATIC_DISTRIBUTION_BUILD_QUEUED_FIELDS: &[FieldSchema] = &[
    field!("distribution_build_id", "uuid"),
    field!("predecessor_build_id", "uuid", optional),
    field!("composition_revision", "uint64"),
    field!("composition_digest", "string"),
    field!("selected_promotions", "uint64"),
];
const MODULE_STATIC_DISTRIBUTION_BUILD_CLAIMED_FIELDS: &[FieldSchema] = &[
    field!("distribution_build_id", "uuid"),
    field!("claim_id", "uuid"),
    field!("attempt_number", "uint64"),
    field!("runner_id", "string"),
    field!("reclaimed_expired_lease", "bool"),
];
const MODULE_STATIC_DISTRIBUTION_BUILD_COMPLETED_FIELDS: &[FieldSchema] = &[
    field!("distribution_build_id", "uuid"),
    field!("claim_id", "uuid"),
    field!("composition_revision", "uint64"),
    field!("composition_digest", "string"),
    field!("outcome", "string"),
    field!("result_digest", "string", optional),
    field!("completion_digest", "string"),
];
const MODULE_STATIC_DISTRIBUTION_RELEASE_ACTIVATED_FIELDS: &[FieldSchema] = &[
    field!("distribution_release_id", "uuid"),
    field!("predecessor_release_id", "uuid", optional),
    field!("distribution_build_id", "uuid"),
    field!("release_revision", "uint64"),
    field!("composition_revision", "uint64"),
    field!("composition_digest", "string"),
    field!("artifact_digest", "string"),
    field!("policy_revision", "string"),
];
const MODULE_STATIC_DISTRIBUTION_ROLLBACK_BUILD_QUEUED_FIELDS: &[FieldSchema] = &[
    field!("rollback_id", "uuid"),
    field!("from_release_id", "uuid"),
    field!("target_release_id", "uuid"),
    field!("distribution_build_id", "uuid"),
    field!("composition_revision", "uint64"),
    field!("composition_digest", "string"),
    field!("policy_revision", "string"),
];
const MODULE_STATIC_DISTRIBUTION_RELEASE_REVOKED_FIELDS: &[FieldSchema] = &[
    field!("distribution_release_id", "uuid"),
    field!("distribution_build_id", "uuid"),
    field!("release_state_revision", "uint64"),
    field!("was_active", "bool"),
    field!("policy_revision", "string"),
];
const MODULE_STATIC_DISTRIBUTION_ROLLOUT_REQUESTED_FIELDS: &[FieldSchema] = &[
    field!("rollout_id", "uuid"),
    field!("predecessor_rollout_id", "uuid", optional),
    field!("distribution_release_id", "uuid"),
    field!("rollout_revision", "uint64"),
    field!("rollout_state_revision", "uint64"),
    field!("composition_revision", "uint64"),
    field!("composition_digest", "string"),
    field!("artifact_digest", "string"),
    field!("topology_digest", "string"),
    field!("policy_revision", "string"),
    field!("target_nodes", "uint64"),
    field!("executor_mode", "string"),
];
const MODULE_STATIC_DISTRIBUTION_NODE_OBSERVED_FIELDS: &[FieldSchema] = &[
    field!("rollout_id", "uuid"),
    field!("node_id", "string"),
    field!("reporter_id", "string"),
    field!("observation_revision", "uint64"),
    field!("phase", "string"),
    field!("report_digest", "string"),
];
const MODULE_STATIC_DISTRIBUTION_ROLLOUT_STATUS_CHANGED_FIELDS: &[FieldSchema] = &[
    field!("rollout_id", "uuid"),
    field!("distribution_release_id", "uuid"),
    field!("rollout_revision", "uint64"),
    field!("rollout_state_revision", "uint64"),
    field!("status", "string"),
    field!("observed_rollout_id", "uuid", optional),
    field!("failure_code", "string", optional),
];
const MODULE_ARTIFACT_SECURITY_STATE_CHANGED_FIELDS: &[FieldSchema] = &[
    field!("module_slug", "string"),
    field!("module_version", "string"),
    field!("payload_digest", "string"),
    field!("security_revision", "uint64"),
    field!("status", "string"),
    field!("policy_revision", "string"),
    field!("reason_code", "string"),
];
const LOCALE_FIELDS: &[FieldSchema] = &[field!("tenant_id", "uuid"), field!("locale", "string")];

pub const EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: "node.created",
        version: 1,
        description: "A content node was created.",
        fields: NODE_CREATED_FIELDS,
    },
    EventSchema {
        event_type: "node.updated",
        version: 1,
        description: "A content node was updated.",
        fields: NODE_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "node.translation.updated",
        version: 1,
        description: "A node translation was updated.",
        fields: NODE_TRANSLATION_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "node.published",
        version: 1,
        description: "A content node was published.",
        fields: NODE_PUBLISHED_FIELDS,
    },
    EventSchema {
        event_type: "node.unpublished",
        version: 1,
        description: "A content node was unpublished.",
        fields: NODE_UNPUBLISHED_FIELDS,
    },
    EventSchema {
        event_type: "node.deleted",
        version: 1,
        description: "A content node was deleted.",
        fields: NODE_DELETED_FIELDS,
    },
    EventSchema {
        event_type: "body.updated",
        version: 1,
        description: "A node body was updated.",
        fields: BODY_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "category.created",
        version: 1,
        description: "A category was created.",
        fields: CATEGORY_ID_FIELDS,
    },
    EventSchema {
        event_type: "category.updated",
        version: 1,
        description: "A category was updated.",
        fields: CATEGORY_ID_FIELDS,
    },
    EventSchema {
        event_type: "category.deleted",
        version: 1,
        description: "A category was deleted.",
        fields: CATEGORY_ID_FIELDS,
    },
    EventSchema {
        event_type: "tag.created",
        version: 1,
        description: "A tag was created.",
        fields: TAG_ID_FIELDS,
    },
    EventSchema {
        event_type: "tag.attached",
        version: 1,
        description: "A tag was attached to a target.",
        fields: TAG_RELATION_FIELDS,
    },
    EventSchema {
        event_type: "tag.detached",
        version: 1,
        description: "A tag was detached from a target.",
        fields: TAG_RELATION_FIELDS,
    },
    EventSchema {
        event_type: "media.uploaded",
        version: 1,
        description: "Media asset uploaded.",
        fields: MEDIA_UPLOADED_FIELDS,
    },
    EventSchema {
        event_type: "media.deleted",
        version: 1,
        description: "Media asset deleted.",
        fields: MEDIA_DELETED_FIELDS,
    },
    EventSchema {
        event_type: "user.registered",
        version: 1,
        description: "A user registered.",
        fields: USER_REGISTERED_FIELDS,
    },
    EventSchema {
        event_type: "user.logged_in",
        version: 1,
        description: "A user logged in.",
        fields: USER_LOGGED_IN_FIELDS,
    },
    EventSchema {
        event_type: "user.updated",
        version: 1,
        description: "A user profile was updated.",
        fields: USER_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "profile.updated",
        version: 1,
        description: "A public profile was updated.",
        fields: PROFILE_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "user.deleted",
        version: 1,
        description: "A user was deleted.",
        fields: USER_DELETED_FIELDS,
    },
    EventSchema {
        event_type: "product.created",
        version: 1,
        description: "A product was created.",
        fields: PRODUCT_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.updated",
        version: 1,
        description: "A product was updated.",
        fields: PRODUCT_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.published",
        version: 1,
        description: "A product was published.",
        fields: PRODUCT_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.deleted",
        version: 1,
        description: "A product was deleted.",
        fields: PRODUCT_ID_FIELDS,
    },
    EventSchema {
        event_type: "variant.created",
        version: 1,
        description: "A product variant was created.",
        fields: VARIANT_FIELDS,
    },
    EventSchema {
        event_type: "variant.updated",
        version: 1,
        description: "A product variant was updated.",
        fields: VARIANT_FIELDS,
    },
    EventSchema {
        event_type: "variant.deleted",
        version: 1,
        description: "A product variant was deleted.",
        fields: VARIANT_FIELDS,
    },
    EventSchema {
        event_type: "inventory.updated",
        version: 1,
        description: "Inventory was updated.",
        fields: INVENTORY_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "inventory.low",
        version: 1,
        description: "Inventory low threshold reached.",
        fields: INVENTORY_LOW_FIELDS,
    },
    EventSchema {
        event_type: "price.updated",
        version: 1,
        description: "Price was updated.",
        fields: PRICE_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "order.placed",
        version: 1,
        description: "Order was placed.",
        fields: ORDER_PLACED_FIELDS,
    },
    EventSchema {
        event_type: "order.status_changed",
        version: 1,
        description: "Order status changed.",
        fields: ORDER_STATUS_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "order.completed",
        version: 1,
        description: "Order completed.",
        fields: ORDER_COMPLETED_FIELDS,
    },
    EventSchema {
        event_type: "order.cancelled",
        version: 1,
        description: "Order cancelled.",
        fields: ORDER_CANCELLED_FIELDS,
    },
    EventSchema {
        event_type: "index.reindex_requested",
        version: 1,
        description: "Index rebuild requested.",
        fields: REINDEX_REQUESTED_FIELDS,
    },
    EventSchema {
        event_type: "index.updated",
        version: 1,
        description: "Index entry updated.",
        fields: INDEX_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "build.requested",
        version: 1,
        description: "Build requested.",
        fields: BUILD_REQUESTED_FIELDS,
    },
    EventSchema {
        event_type: "build.rolled_back",
        version: 1,
        description: "An active platform build release was rolled back to its direct predecessor.",
        fields: BUILD_ROLLED_BACK_FIELDS,
    },
    EventSchema {
        event_type: "blog.post.created",
        version: 1,
        description: "Blog post created.",
        fields: BLOG_POST_CREATED_FIELDS,
    },
    EventSchema {
        event_type: "blog.post.published",
        version: 1,
        description: "Blog post published.",
        fields: BLOG_POST_PUBLISHED_FIELDS,
    },
    EventSchema {
        event_type: "blog.post.unpublished",
        version: 1,
        description: "Blog post unpublished.",
        fields: BLOG_POST_UNPUBLISHED_FIELDS,
    },
    EventSchema {
        event_type: "blog.post.updated",
        version: 1,
        description: "Blog post updated.",
        fields: BLOG_POST_UPDATED_FIELDS,
    },
    EventSchema {
        event_type: "blog.post.archived",
        version: 1,
        description: "Blog post archived.",
        fields: BLOG_POST_ARCHIVED_FIELDS,
    },
    EventSchema {
        event_type: "blog.post.deleted",
        version: 1,
        description: "Blog post deleted.",
        fields: BLOG_POST_DELETED_FIELDS,
    },
    EventSchema {
        event_type: "comment.created",
        version: 1,
        description: "Comment created.",
        fields: COMMENT_FIELDS,
    },
    EventSchema {
        event_type: "comment.deleted",
        version: 1,
        description: "Comment deleted.",
        fields: COMMENT_FIELDS,
    },
    EventSchema {
        event_type: "forum.topic.created",
        version: 1,
        description: "Forum topic created.",
        fields: FORUM_TOPIC_CREATED_FIELDS,
    },
    EventSchema {
        event_type: "forum.topic.replied",
        version: 1,
        description: "Forum topic replied.",
        fields: FORUM_TOPIC_REPLIED_FIELDS,
    },
    EventSchema {
        event_type: "forum.topic.status_changed",
        version: 1,
        description: "Forum topic status changed.",
        fields: FORUM_TOPIC_STATUS_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "forum.topic.pinned",
        version: 1,
        description: "Forum topic pinned state changed.",
        fields: FORUM_TOPIC_PINNED_FIELDS,
    },
    EventSchema {
        event_type: "forum.reply.status_changed",
        version: 1,
        description: "Forum reply status changed.",
        fields: FORUM_REPLY_STATUS_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "content.topic.promoted_to_post",
        version: 1,
        description: "Forum topic promoted to blog post.",
        fields: TOPIC_PROMOTED_TO_POST_FIELDS,
    },
    EventSchema {
        event_type: "content.post.demoted_to_topic",
        version: 1,
        description: "Blog post demoted to forum topic.",
        fields: POST_DEMOTED_TO_TOPIC_FIELDS,
    },
    EventSchema {
        event_type: "content.topic.split",
        version: 1,
        description: "Forum topic split.",
        fields: TOPIC_SPLIT_FIELDS,
    },
    EventSchema {
        event_type: "content.topics.merged",
        version: 1,
        description: "Forum topics merged.",
        fields: TOPICS_MERGED_FIELDS,
    },
    EventSchema {
        event_type: "content.canonical_url.changed",
        version: 1,
        description: "Canonical URL mapping changed or was reasserted for a content target.",
        fields: CANONICAL_URL_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "content.url_alias.purged",
        version: 1,
        description: "Legacy URL aliases must be purged from index and cache layers.",
        fields: URL_ALIAS_PURGED_FIELDS,
    },
    EventSchema {
        event_type: "seo.meta.upserted",
        version: 1,
        description: "SEO metadata was upserted for a target/locale scope.",
        fields: SEO_META_UPSERTED_FIELDS,
    },
    EventSchema {
        event_type: "seo.revision.published",
        version: 1,
        description: "SEO revision snapshot was published.",
        fields: SEO_REVISION_FIELDS,
    },
    EventSchema {
        event_type: "seo.revision.rolled_back",
        version: 1,
        description: "SEO metadata was rolled back to a prior revision.",
        fields: SEO_REVISION_FIELDS,
    },
    EventSchema {
        event_type: "seo.redirect.upserted",
        version: 1,
        description: "SEO redirect entry was created or updated.",
        fields: SEO_REDIRECT_UPSERTED_FIELDS,
    },
    EventSchema {
        event_type: "seo.redirect.disabled",
        version: 1,
        description: "SEO redirect entry was explicitly disabled.",
        fields: SEO_REDIRECT_DISABLED_FIELDS,
    },
    EventSchema {
        event_type: "seo.sitemap.generated",
        version: 1,
        description: "Sitemap generation job finished writing sitemap artifacts.",
        fields: SEO_SITEMAP_GENERATED_FIELDS,
    },
    EventSchema {
        event_type: "seo.sitemap.submitted",
        version: 1,
        description: "Sitemap submission fan-out to external endpoints finished.",
        fields: SEO_SITEMAP_SUBMITTED_FIELDS,
    },
    EventSchema {
        event_type: "seo.bulk.completed",
        version: 1,
        description: "SEO bulk job completed without item failures.",
        fields: SEO_BULK_COMPLETED_FIELDS,
    },
    EventSchema {
        event_type: "seo.bulk.partial",
        version: 1,
        description: "SEO bulk job reached a terminal state with mixed successes and failures.",
        fields: SEO_BULK_COMPLETED_FIELDS,
    },
    EventSchema {
        event_type: "seo.bulk.failed",
        version: 1,
        description: "SEO bulk job reached a failed terminal state.",
        fields: SEO_BULK_COMPLETED_FIELDS,
    },
    EventSchema {
        event_type: "tenant.created",
        version: 1,
        description: "Tenant created.",
        fields: TENANT_ID_FIELDS,
    },
    EventSchema {
        event_type: "tenant.updated",
        version: 1,
        description: "Tenant updated.",
        fields: TENANT_ID_FIELDS,
    },
    EventSchema {
        event_type: "tenant.module.toggled",
        version: 1,
        description: "Tenant module toggle state changed.",
        fields: TENANT_MODULE_TOGGLED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.admitted",
        version: 1,
        description: "An admitted module artifact was committed with its control-plane metadata.",
        fields: MODULE_ARTIFACT_ADMITTED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.reverified",
        version: 1,
        description: "Module artifact trust evidence was reverified.",
        fields: MODULE_ARTIFACT_REVERIFIED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_promotion.requested",
        version: 1,
        description: "A platform-built module release was submitted for static promotion review.",
        fields: MODULE_STATIC_PROMOTION_REQUESTED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_promotion.approved",
        version: 1,
        description: "A reviewed static promotion was approved under an immutable policy.",
        fields: MODULE_STATIC_PROMOTION_APPROVED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.build_queued",
        version: 1,
        description: "An immutable static distribution composition build was queued.",
        fields: MODULE_STATIC_DISTRIBUTION_BUILD_QUEUED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.build_claimed",
        version: 1,
        description: "A static distribution build attempt acquired its bounded worker lease.",
        fields: MODULE_STATIC_DISTRIBUTION_BUILD_CLAIMED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.build_completed",
        version: 1,
        description: "A static distribution build recorded immutable terminal evidence.",
        fields: MODULE_STATIC_DISTRIBUTION_BUILD_COMPLETED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.release_activated",
        version: 1,
        description: "A verified static distribution build became the current release head.",
        fields: MODULE_STATIC_DISTRIBUTION_RELEASE_ACTIVATED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.rollback_build_queued",
        version: 1,
        description: "A direct-predecessor rollback queued a new immutable distribution build.",
        fields: MODULE_STATIC_DISTRIBUTION_ROLLBACK_BUILD_QUEUED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.release_revoked",
        version: 1,
        description: "A static distribution release was revoked under an immutable policy.",
        fields: MODULE_STATIC_DISTRIBUTION_RELEASE_REVOKED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.rollout_requested",
        version: 1,
        description: "A topology-bound native distribution rollout became desired.",
        fields: MODULE_STATIC_DISTRIBUTION_ROLLOUT_REQUESTED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.node_observed",
        version: 1,
        description: "A deployment node reported an exact native distribution observation.",
        fields: MODULE_STATIC_DISTRIBUTION_NODE_OBSERVED_FIELDS,
    },
    EventSchema {
        event_type: "module.static_distribution.rollout_status_changed",
        version: 1,
        description: "A native distribution rollout changed durable observed status.",
        fields: MODULE_STATIC_DISTRIBUTION_ROLLOUT_STATUS_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.security_state_changed",
        version: 1,
        description: "An immutable artifact release changed global quarantine or revocation state.",
        fields: MODULE_ARTIFACT_SECURITY_STATE_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "locale.enabled",
        version: 1,
        description: "Locale enabled for tenant.",
        fields: LOCALE_FIELDS,
    },
    EventSchema {
        event_type: "locale.disabled",
        version: 1,
        description: "Locale disabled for tenant.",
        fields: LOCALE_FIELDS,
    },
    // ── Flex — field definition events ──────────────────────────────────
    EventSchema {
        event_type: "field_definition.created",
        version: 1,
        description: "A custom field definition was created for an entity type.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "entity_type",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "field_key",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "field_type",
                data_type: "string",
                optional: false,
            },
        ],
    },
    EventSchema {
        event_type: "field_definition.updated",
        version: 1,
        description: "A custom field definition was updated.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "entity_type",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "field_key",
                data_type: "string",
                optional: false,
            },
        ],
    },
    EventSchema {
        event_type: "field_definition.deleted",
        version: 1,
        description: "A custom field definition was soft-deleted.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "entity_type",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "field_key",
                data_type: "string",
                optional: false,
            },
        ],
    },
    EventSchema {
        event_type: "flex.schema.created",
        version: 1,
        description: "A standalone flex schema was created.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "schema_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "slug",
                data_type: "string",
                optional: false,
            },
        ],
    },
    EventSchema {
        event_type: "flex.schema.updated",
        version: 1,
        description: "A standalone flex schema was updated.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "schema_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "slug",
                data_type: "string",
                optional: false,
            },
        ],
    },
    EventSchema {
        event_type: "flex.schema.deleted",
        version: 1,
        description: "A standalone flex schema was deleted.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "schema_id",
                data_type: "string",
                optional: false,
            },
        ],
    },
    EventSchema {
        event_type: "flex.entry.created",
        version: 1,
        description: "A standalone flex entry was created.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "schema_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "entry_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "entity_type",
                data_type: "string",
                optional: true,
            },
            FieldSchema {
                name: "entity_id",
                data_type: "string",
                optional: true,
            },
        ],
    },
    EventSchema {
        event_type: "flex.entry.updated",
        version: 1,
        description: "A standalone flex entry was updated.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "schema_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "entry_id",
                data_type: "string",
                optional: false,
            },
        ],
    },
    EventSchema {
        event_type: "flex.entry.deleted",
        version: 1,
        description: "A standalone flex entry was deleted.",
        fields: &[
            FieldSchema {
                name: "tenant_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "schema_id",
                data_type: "string",
                optional: false,
            },
            FieldSchema {
                name: "entry_id",
                data_type: "string",
                optional: false,
            },
        ],
    },
    EventSchema {
        event_type: "product.attribute.created",
        version: 1,
        description: "A product attribute was created.",
        fields: ATTRIBUTE_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute.updated",
        version: 1,
        description: "A product attribute was updated.",
        fields: ATTRIBUTE_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute.deleted",
        version: 1,
        description: "A product attribute was deleted.",
        fields: ATTRIBUTE_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute_option.created",
        version: 1,
        description: "A product attribute option was created.",
        fields: ATTRIBUTE_OPTION_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute_option.updated",
        version: 1,
        description: "A product attribute option was updated.",
        fields: ATTRIBUTE_OPTION_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute_option.deleted",
        version: 1,
        description: "A product attribute option was deleted.",
        fields: ATTRIBUTE_OPTION_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute_schema.created",
        version: 1,
        description: "A product attribute schema was created.",
        fields: ATTRIBUTE_SCHEMA_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute_schema.updated",
        version: 1,
        description: "A product attribute schema was updated.",
        fields: ATTRIBUTE_SCHEMA_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute_schema.deleted",
        version: 1,
        description: "A product attribute schema was deleted.",
        fields: ATTRIBUTE_SCHEMA_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute_schema.bindings_changed",
        version: 1,
        description: "Product attribute schema bindings changed.",
        fields: ATTRIBUTE_SCHEMA_FIELDS,
    },
    EventSchema {
        event_type: "catalog.category.created",
        version: 1,
        description: "A catalog category was created.",
        fields: CATEGORY_ID_FIELDS,
    },
    EventSchema {
        event_type: "catalog.category.updated",
        version: 1,
        description: "A catalog category was updated.",
        fields: CATEGORY_ID_FIELDS,
    },
    EventSchema {
        event_type: "catalog.category.deleted",
        version: 1,
        description: "A catalog category was deleted.",
        fields: CATEGORY_ID_FIELDS,
    },
    EventSchema {
        event_type: "catalog.category.schema_mode_changed",
        version: 1,
        description: "A catalog category schema mode changed.",
        fields: CATEGORY_ID_FIELDS,
    },
    EventSchema {
        event_type: "catalog.category.attributes_changed",
        version: 1,
        description: "Catalog category attribute bindings changed.",
        fields: CATEGORY_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.primary_category.changed",
        version: 1,
        description: "A product primary category changed.",
        fields: PRODUCT_PRIMARY_CATEGORY_FIELDS,
    },
    EventSchema {
        event_type: "product.category_assignments.changed",
        version: 1,
        description: "Product category assignments changed.",
        fields: PRODUCT_ID_FIELDS,
    },
    EventSchema {
        event_type: "product.attribute_values.changed",
        version: 1,
        description: "Product attribute values changed.",
        fields: PRODUCT_ID_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.rolled_back",
        version: 1,
        description: "A module artifact installation was rolled back.",
        fields: MODULE_ARTIFACT_ROLLED_BACK_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.uninstalled",
        version: 1,
        description: "A module artifact installation was uninstalled.",
        fields: MODULE_ARTIFACT_REVISION_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.migration_checkpointed",
        version: 1,
        description: "A module artifact migration checkpoint was recorded.",
        fields: MODULE_ARTIFACT_MIGRATION_CHECKPOINTED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.deactivated",
        version: 1,
        description: "A module artifact installation was deactivated.",
        fields: MODULE_ARTIFACT_REVISION_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.tenant_disabled",
        version: 1,
        description: "A module artifact was disabled for a tenant.",
        fields: MODULE_ARTIFACT_TENANT_REVISION_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.tenant_enabled",
        version: 1,
        description: "A module artifact was enabled for a tenant.",
        fields: MODULE_ARTIFACT_TENANT_REVISION_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.data_purged",
        version: 1,
        description: "Module artifact tenant data was purged.",
        fields: MODULE_ARTIFACT_DATA_PURGED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.data_exported",
        version: 1,
        description: "Module artifact tenant data was exported.",
        fields: MODULE_ARTIFACT_DATA_EXPORTED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.data_snapshot_created",
        version: 1,
        description: "A module artifact data snapshot was created.",
        fields: MODULE_ARTIFACT_DATA_SNAPSHOT_CREATED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.data_snapshot_restored",
        version: 1,
        description: "A module artifact data snapshot was restored.",
        fields: MODULE_ARTIFACT_DATA_SNAPSHOT_RESTORED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.data_snapshot_retention_updated",
        version: 1,
        description: "A module artifact data snapshot retention policy changed.",
        fields: MODULE_ARTIFACT_DATA_SNAPSHOT_RETENTION_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.data_snapshot_collected",
        version: 1,
        description: "A module artifact data snapshot was collected.",
        fields: MODULE_ARTIFACT_DATA_SNAPSHOT_COLLECTED_FIELDS,
    },
    EventSchema {
        event_type: "module.artifact.secret_bound",
        version: 1,
        description: "A module artifact secret binding changed.",
        fields: MODULE_ARTIFACT_SECRET_BOUND_FIELDS,
    },
    EventSchema {
        event_type: "module.build.queued",
        version: 1,
        description: "A module build was queued.",
        fields: MODULE_BUILD_QUEUED_FIELDS,
    },
    EventSchema {
        event_type: "module.build.completed",
        version: 1,
        description: "A module build completed.",
        fields: MODULE_BUILD_COMPLETED_FIELDS,
    },
    EventSchema {
        event_type: "module.effective_policy_revision_changed",
        version: 1,
        description: "An effective module policy revision changed.",
        fields: MODULE_EFFECTIVE_POLICY_REVISION_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "platform_settings.changed",
        version: 1,
        description: "Platform settings changed.",
        fields: PLATFORM_SETTINGS_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "search.settings_changed",
        version: 1,
        description: "Search settings changed.",
        fields: SEARCH_SETTINGS_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: "search.rebuild_queued",
        version: 1,
        description: "A search rebuild was queued.",
        fields: SEARCH_REBUILD_QUEUED_FIELDS,
    },
];

pub fn event_schema(event_type: &str) -> Option<&'static EventSchema> {
    EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}
