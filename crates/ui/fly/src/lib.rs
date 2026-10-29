//! # Fly Visual Editor Engine
//!
//! Framework-neutral page-builder engine and lossless GrapesJS compatibility model.
//!
//! The crate is deliberately independent from UI frameworks, browser APIs, RusTok modules,
//! persistence, and transports. Consumers own persistence and framework adapters.

// The entire Fly stack is safe Rust. `forbid` (not `deny`) so it cannot be re-enabled
// locally with an `allow` attribute.
#![forbid(unsafe_code)]

mod action;
mod asset;
mod audit;
mod binding;
mod bundle;
mod codec;
mod command;
mod component_index;
mod component_visit;
mod context_compatibility;
mod context_contract;
mod context_dependency;
mod context_json_schema;
mod context_migration;
mod context_schema;
mod digest;
mod dynamic;
mod embed;
mod error;
mod fragment;
mod id_reference;
mod ids;
mod interaction_capability;
mod interaction_route;
mod internal_link;
mod locale_coverage;
mod locale_policy;
mod locale_resolver;
mod localized_route;
mod model;
mod page;
mod page_metadata;
mod page_metadata_locale;
mod placement;
mod registry;
mod render;
mod rich_text;
mod runtime_locale;
mod runtime_pipeline;
mod runtime_render;
mod runtime_validation;
mod safe_url;
mod snapshot;
mod style_rule;
mod symbol;
mod theme;
mod trait_model;
mod translation;
mod validation;

pub use action::{
    ActionMaterialization, AnimationKind, ComponentAction, ComponentForm,
    FLY_ACTION_DATA_ATTRIBUTE, FLY_ACTION_FIELD, FLY_ACTION_KIND_ATTRIBUTE, FLY_FORM_FIELD,
    FormEncoding, FormMethod, FormProviderEndpointResolver, SlideDirection, VisibilityMode,
    materialize_component_actions, materialize_component_actions_with_providers,
    validate_component_actions,
};
pub use asset::{
    AssetCatalog, AssetDescriptor, AssetKind, AssetPolicy, AssetReference, source_allowed,
};
pub use audit::{AuditDiagnostic, AuditReport, AuditSeverity, audit_page};
pub use binding::{
    BindingCatalog, BindingCommand, BindingMaterialization, BindingTarget, BindingTransform,
    FLY_RUNTIME_BINDINGS_FIELD, RuntimeBinding, apply_binding_command, materialize_bindings,
    validate_binding_definitions,
};
pub use bundle::{
    BundleDecodePolicy, BundleInspection, BundleMetadata, DecodedProjectBundle, ProjectBundle,
    bundle_hash, decode_project_bundle, decode_project_bundle_value, encode_project_bundle,
    export_project_bundle, inspect_project_bundle,
};
pub use codec::{GrapesJsCodec, MAXIMUM_DECODE_DEPTH};
pub use command::{
    AssetCommand, ComponentPatch, DEFAULT_HISTORY_MEMORY_BUDGET_BYTES, EditorCommand, FlyEditor,
    History, HistoryEntry, ProjectHash, RevisionState, SymbolCommand, invert_command,
};
pub use component_index::ComponentIndex;
pub use component_visit::{ComponentVisit, visit_project_components};
pub use context_compatibility::{
    FLY_RUNTIME_CONTEXT_COMPATIBILITY, RuntimeContextCompatibilityPolicy,
    RuntimeContextCompatibilityReport, RuntimeContextContractChange,
    RuntimeContextContractChangeKind, diff_runtime_context_contracts,
    diff_runtime_context_contracts_with_policy,
};
pub use context_contract::{
    RuntimeComputedContract, RuntimeContextContract, RuntimeContextFieldContract,
    RuntimeContextPreflight, RuntimeContextPreflightPolicy, context_expression_dependencies,
    extract_runtime_context_contract, is_valid_runtime_context_path, preflight_runtime_context,
};
pub use context_dependency::{
    RuntimeContextConsumer, RuntimeContextDependencyEdge, RuntimeContextDependencyGraph,
    RuntimeContextDependencyNode, RuntimeContextPathSource, analyze_runtime_context_dependencies,
};
pub use context_json_schema::{
    RuntimeContextExample, RuntimeContextExamplePolicy, RuntimeContextJsonSchema,
    export_runtime_context_json_schema, generate_runtime_context_example,
};
pub use context_migration::{
    RuntimeContextMigration, RuntimeContextMigrationPolicy, migrate_runtime_context,
};
pub use context_schema::{
    ComputedContextValue, ContextCommand, ContextExpression, ContextFieldDefinition,
    ContextMaterialization, ContextSchemaCatalog, ContextValueKind, FLY_RUNTIME_COMPUTED_FIELD,
    FLY_RUNTIME_CONTEXT_SCHEMA_FIELD, apply_context_command, is_valid_context_path,
    materialize_context, remove_context_path, resolve_context_path, set_context_path,
    validate_context_definitions,
};
pub use digest::{ContentDigest, FLY_DIGEST_ALGORITHM, constant_time_eq, sha256_hex};
pub use dynamic::{
    ConditionOperator, DEFAULT_REPEATER_LIMIT, DynamicCatalog, DynamicCommand,
    EmptyRepeaterBehavior, FLY_RUNTIME_CONDITIONS_FIELD, FLY_RUNTIME_REPEATERS_FIELD,
    MAX_REPEATER_DEPTH, MAX_REPEATER_LIMIT, MAX_TOTAL_REPEATED_NODES, RuntimeCondition,
    RuntimeMaterialization, RuntimeRepeater, apply_dynamic_command, materialize_runtime,
    validate_dynamic_definitions,
};
pub use embed::{
    EMBED_IFRAME_SANDBOX, EmbedProvider, EmbedProviderRegistry, EmbedSpec, FLY_EMBED_FIELD,
    builtin_embed_providers, decode_embed_spec, validate_embed_url,
};
pub use error::FlyError;
pub use fragment::{ProjectFragment, ProviderRequirement, RichTextPayload};
pub use ids::{IdGenerator, SequentialIdGenerator, UniqueIdGenerator};
pub use interaction_capability::{
    InteractionCapabilityDefinition, InteractionCapabilityKind, InteractionCapabilityPolicy,
    InteractionCapabilityRegistry, InteractionRuntimeTarget, MissingInteractionCapabilityPolicy,
    validate_component_actions_with_capabilities, validate_interaction_capabilities,
};
pub use internal_link::{
    FLY_PAGE_LINK_FIELD, InternalLinkMaterialization, InternalPageLink,
    materialize_internal_page_links, validate_internal_page_links,
};
pub use locale_coverage::{
    LocaleCoverageGap, LocaleCoverageKind, LocaleCoverageReport, LocaleCoverageSummary,
    analyze_project_locale_coverage,
};
pub use locale_policy::{
    FLY_LOCALES_FIELD, LocalePolicyMaterialization, ProjectLocalePolicy,
    clear_project_locale_policy, materialize_project_locale_context, set_project_locale_policy,
    validate_project_locale_policy,
};
#[cfg(feature = "platform-i18n")]
pub use locale_resolver::PlatformLocaleResolver;
pub use locale_resolver::{
    BasicLocaleResolver, DefaultLocaleResolver, LocaleResolver, default_locale_resolver,
};
pub use localized_route::{
    LocalizedPageRouteEntry, LocalizedPageRouteResolution, localized_page_route_index,
    resolve_localized_page_route, validate_localized_page_routes,
};
pub use model::{
    ComponentChildren, ComponentNode, ComponentObject, DocumentKind, FLY_DOCUMENT_KIND_FIELD,
    GrapesProject, ProjectDocument, ProjectPage,
};
pub use page::{PageCommand, PageLocator, PagePatch, PageSummary, apply_page_command, blank_page};
pub use page_metadata::{FLY_PAGE_METADATA_FIELD, PageMetadata, normalize_slug};
pub use page_metadata_locale::{
    LocalizedPageMetadataMaterialization, materialize_localized_page_metadata,
};
pub use placement::{ComponentLocation, PlacementDecision};
pub use registry::{
    AssetProviderDefinition, BlockDefinition, CommandDefinition, ComponentDefinition,
    PluginDescriptor, PluginRequirement, Registry, RegistryItem, RegistrySet, SelectorDefinition,
    StyleDefinition, TraitDefinition, builtin_blocks, builtin_commands,
    builtin_component_definitions,
};
pub use render::{
    AlternateLocaleLink, PageHead, PageSelection, RenderPolicy, RenderedPage,
    compose_document_html, render_page, render_page_with_rich_text, resolve_page,
};
pub use rich_text::{
    FLY_RICH_TEXT_FIELD, MAXIMUM_RICH_TEXT_BYTES, MAXIMUM_RICH_TEXT_DEPTH, MAXIMUM_RICH_TEXT_NODES,
    RICH_TEXT_CAPABILITY, RichTextHtmlRenderer, decode_rich_text_payload, rich_text_fallback_html,
    rich_text_plain_text, validate_rich_text_payload,
};
pub use runtime_locale::{
    LOCALIZED_FALLBACK_FIELD, LOCALIZED_VALUES_FIELD, RUNTIME_FALLBACK_LOCALES_FIELD,
    RUNTIME_LOCALE_FIELD, RuntimeLocaleMaterialization, RuntimeLocaleSelection,
    materialize_runtime_locale_context, normalize_locale_tag,
};
pub use runtime_pipeline::{
    RuntimeProjectMaterialization, materialize_project_with_runtime_context,
    materialize_project_with_runtime_context_with_providers,
};
pub use runtime_render::{
    RuntimeRenderResult, render_page_with_runtime_context,
    render_page_with_runtime_context_with_providers,
};
pub use runtime_validation::{extend_with_runtime_validation, validate_runtime_extensions};
pub use snapshot::{ProjectDiffSummary, ProjectSnapshot, SnapshotCatalog, compare_projects};
pub use style_rule::{
    FLY_COMPONENT_RULE_FIELD, FLY_RULE_ID_FIELD, SUPPORTED_PSEUDO_CLASSES, StyleRuleCatalog,
    StyleRuleCommand, StyleRuleDescriptor, StyleRuleIdentity, StyleRuleScope,
    apply_style_rule_command, component_rule_value, is_supported_pseudo_class,
    normalize_pseudo_class,
};
pub use symbol::{
    FLY_SYMBOLS_FIELD, MAX_SYMBOL_COMPONENTS, MAX_SYMBOL_DEFINITION_BYTES, MAX_SYMBOL_DEFINITIONS,
    SymbolDescriptor, apply_symbol_command, convert_component_to_symbol,
    document_has_symbol_instances, insert_symbol_instance, resolve_symbol_instances,
    set_symbol_descriptors, symbol_reference_cycles, symbol_references,
};
pub use theme::{
    DesignToken, FLY_THEME_FIELD, MAXIMUM_THEME_TOKENS, MAXIMUM_TOKEN_NAME_LENGTH,
    MAXIMUM_TOKEN_VALUE_LENGTH, ThemeCommand, apply_theme_command, decode_theme_tokens,
    theme_root_rule, validate_token_name, validate_token_value,
};
pub use trait_model::{
    TraitOption, TraitSchema, TraitSchemaRegistry, TraitSnapshot, TraitTarget, TraitValueKind,
    builtin_trait_schemas, trait_snapshots,
};
pub use translation::{
    FLY_TRANSLATIONS_FIELD, RUNTIME_TRANSLATIONS_CONTEXT_FIELD, TranslationCatalog,
    TranslationCommand, TranslationEntry, TranslationMaterialization, apply_translation_command,
    materialize_project_translations, validate_translation_definitions,
};
pub use validation::{
    DEFAULT_MAXIMUM_DEPTH, DEFAULT_MAXIMUM_NODES, MAXIMUM_IDENTIFIER_LENGTH, ValidationDiagnostic,
    ValidationLimits, ValidationReport, ValidationSeverity, validate_component_public_urls,
    validate_identifier, validate_patch_incremental, validate_project,
};

pub const GRAPESJS_FORMAT: &str = "grapesjs";
pub const FLY_FRAGMENT_FORMAT: &str = "fly_fragment";
pub const RICH_TEXT_PAYLOAD_FORMAT: &str = "fly_rich_text_payload";

pub type FlyResult<T> = Result<T, FlyError>;

#[cfg(test)]
mod tests;
