pub mod async_utils;
pub mod auth;
pub mod cache;
pub mod config;
pub mod content_format;
pub mod context;
pub mod error;
pub mod events;
pub mod field_schema;
pub mod health;
pub mod i18n;
pub mod id;
pub mod metrics;
pub mod migrations;
pub mod module;
pub mod permissions;
pub mod rbac;
pub mod registry;
pub mod resilience;
pub mod rt_json;
pub mod scripting;
pub mod security;
pub mod state_machine;
pub mod tenant_validation;
pub mod tracing;
pub mod typed_error;
pub mod types;
pub mod utils;

#[cfg(test)]
mod validation_proptest;
pub use async_utils::{
    batch, parallel, retry, timeout, BackoffConfig, Coalescer, Debouncer, RetryError, Throttler,
    TimeoutError,
};
pub use auth::{
    AuthError, IdentityService, IdentityTokens, RegistrationInput, User, UserRepository,
    UsersMigration,
};
#[cfg(feature = "redis-cache")]
pub use cache::RedisCacheBackend;
pub use cache::{CacheStats, FallbackCacheBackend, InMemoryCacheBackend};
pub use config::{
    Config, ConfigError, ConfigLoader, ConfigSource, ConfigValue, DatabaseConfig, Secret,
    ServerConfig,
};
pub use content_format::{
    normalize_content_format, prepare_content_payload, PreparedContent, CONTENT_FORMAT_MARKDOWN,
    CONTENT_FORMAT_RT_JSON_V1,
};
pub use context::{AppContext, CacheBackend, SearchBackend};
pub use error::{
    Error, ErrorContext, ErrorKind, ErrorResponse, FieldError, Result, RichError,
    ValidationErrorBuilder,
};
pub use field_schema::{
    create_field_definitions_table, drop_field_definitions_table, is_valid_field_key,
    is_valid_locale_key, CustomFieldsSchema, FieldDefinition, FieldErrorCode, FieldType,
    FieldValidationError, FlexError, HasCustomFields, SelectOption, ValidationRule,
};
pub use events::{
    event_schema, DispatcherConfig, DomainEvent, EventBus, EventBusStats, EventConsumerRuntime,
    EventDispatcher, EventEnvelope, EventHandler, EventSchema, EventTransport, FieldSchema,
    HandlerBuilder, HandlerResult, MemoryTransport, ReliabilityLevel, RunningDispatcher,
    EVENT_SCHEMAS,
};
pub use health::{
    checks::{DatabaseHealthCheck, FnHealthCheck},
    HealthCheck, HealthRegistry, HealthResult, HealthStatus, OverallHealth,
};
pub use i18n::{extract_locale_from_header, translate, Locale};
pub use id::generate_id;
pub use metrics::{Counter, Gauge, Histogram, MetricSnapshot, MetricValue, MetricsRegistry, Timer};
pub use migrations::ModuleMigration;
pub use module::{EventListener, MigrationSource};
pub use module::{ModuleContext, ModuleKind, RusToKModule};
pub use permissions::{Action, Permission, Resource};
pub use rbac::{PermissionScope, Rbac, SecurityContext};
pub use registry::ModuleRegistry;
pub use resilience::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitState, RetryPolicy,
    RetryStrategy,
};
pub use rt_json::{
    sanitize_rt_json_before_html_render, validate_and_sanitize_rt_json, RtJsonValidationConfig,
    RtJsonValidationResult,
};
pub use scripting::ScriptingContext;
pub use security::{
    audit::AuditEventType, headers::FrameOptions, run_security_audit, AuditEvent, AuditLogger,
    InputValidator, RateLimitConfig, RateLimitResult, RateLimiter, SecurityAudit,
    SecurityAuditResult, SecurityCategory, SecurityConfig, SecurityFinding, SecurityHeaders,
    SecurityHeadersConfig, Severity, SsrfProtection, ValidationResult,
};
pub use typed_error::{
    DomainError, ErrorCategory, ErrorCode, ErrorResponseBody, IntoTypedResult, TypedResult,
};
pub use types::{UserRole, UserStatus};
pub use utils::{
    all, any, base64_decode, base64_encode, capitalize, chunk, collect_results, dedup, filter_map,
    find_first, format_duration, get_or_default, group_by, hex_decode, hex_encode, html_escape,
    is_valid_email, is_valid_url, is_valid_uuid, merge_maps, now_millis, now_seconds, parse_bool,
    parse_duration, partition, pluralize, random_string, simple_hash, slugify, to_camel_case,
    to_snake_case, truncate,
};

pub mod prelude {
    pub use crate::field_schema::{
        CustomFieldsSchema, FieldDefinition, FieldType, HasCustomFields,
    };
    pub use crate::async_utils::{batch, parallel, retry, BackoffConfig, RetryError, Throttler};
    pub use crate::config::{ConfigLoader, ConfigSource, Secret};
    pub use crate::domain_err;
    pub use crate::error::{Error, Result};
    pub use crate::events::{
        event_schema, DispatcherConfig, DomainEvent, EventBus, EventBusStats, EventConsumerRuntime,
        EventDispatcher, EventEnvelope, EventHandler, EventSchema, EventTransport, FieldSchema,
        HandlerBuilder, HandlerResult, MemoryTransport, ReliabilityLevel, RunningDispatcher,
        EVENT_SCHEMAS,
    };
    pub use crate::health::{
        HealthCheck, HealthRegistry, HealthResult, HealthStatus, OverallHealth,
    };
    pub use crate::id::generate_id;
    pub use crate::metrics::{Counter, Gauge, Histogram, MetricsRegistry, Timer};
    pub use crate::permissions::{Action, Permission, Resource};
    pub use crate::rbac::{PermissionScope, Rbac, SecurityContext};
    pub use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
    pub use crate::typed_error::{DomainError, ErrorCode, TypedResult};
    pub use crate::types::{UserRole, UserStatus};
    #[cfg(feature = "redis-cache")]
    pub use crate::RedisCacheBackend;
    pub use crate::{
        AppContext, CacheBackend, CacheStats, FallbackCacheBackend, InMemoryCacheBackend,
        SearchBackend,
    };
    pub use uuid::Uuid;
}

#[cfg(test)]
mod contract_tests;
