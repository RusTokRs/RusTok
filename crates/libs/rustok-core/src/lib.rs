/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Core
//!
//! Platform foundation runtime library for the RusToK platform.
//!
//! `rustok-core` defines the minimal platform-wide runtime contracts, kernel types,
//! and foundational primitives used across the host server (`apps/server`) and all
//! sovereign domain modules. It owns foundational runtime abstractions while staying
//! decoupled from host-specific transports and domain-specific business rules.
//!
//! ## Core Architecture & Capabilities
//!
//! - **Module Lifecycle & Registry**: [`module`] ([`RusToKModule`]) and [`registry`] ([`ModuleRegistry`])
//!   provide registration, migration sources, and event listener hooks for domain modules.
//! - **Runtime Context & Caching**: [`context`] ([`AppContext`], [`CacheBackend`], [`SearchBackend`])
//!   and [`InMemoryCacheBackend`] provide multi-tenant runtime state and atomic CAS caching.
//! - **Event Bus & Dispatching**: [`events`] ([`EventBus`], [`EventDispatcher`], [`MemoryTransport`])
//!   and backpressure controls ([`BackpressureController`]) provide decoupled in-process messaging.
//! - **Identity & Security**: [`id`] ([`generate_id`], [`parse_id`]), [`security`] ([`RateLimiter`],
//!   [`InputValidator`], [`SecurityHeaders`]), and [`rbac`] ([`Rbac`], [`SecurityContext`]).
//! - **Resilience Primitives**: [`resilience`] ([`CircuitBreaker`], [`RetryPolicy`]) and [`async_utils`]
//!   ([`retry`], [`timeout`], [`batch`], [`parallel`], [`Throttler`], [`Debouncer`]).
//! - **Canonical Money**: [`money`] provides platform-wide ISO 4217 currency exponent normalization,
//!   rounding, and major/minor unit conversions.
//! - **Flex & Custom Fields**: [`field_schema`] ([`CustomFieldsSchema`], [`FieldDefinition`]) provides
//!   schema-driven extensible field validation and JSON operators.
//! - **Typed Errors**: [`typed_error`] ([`DomainError`], [`ErrorCode`], [`TypedResult`]) and [`error`]
//!   ([`Error`], [`RichError`]).

pub mod async_utils;
mod cache;
mod cache_atomic;
pub mod config;
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
pub mod money;
pub mod rbac;
pub mod registry;
pub mod resilience;
pub mod retention;
pub mod search_projection;
pub mod security;
pub mod security_principal;
pub mod state_machine;
pub mod tenant_validation;
pub mod tracing;
pub mod typed_error;
pub mod types;
pub mod utils;

#[cfg(test)]
mod validation_proptest;
pub use async_utils::{
    BackoffConfig, Coalescer, Debouncer, RetryError, Throttler, TimeoutError, batch, parallel,
    retry, timeout,
};
pub use cache::CacheStats;
pub use cache_atomic::InMemoryCacheBackend;
pub use config::{
    Config, ConfigError, ConfigLoader, ConfigSource, ConfigValue, DatabaseConfig, Secret,
    ServerConfig,
};
pub use context::{AppContext, CacheBackend, CacheCompareAndSetOutcome, SearchBackend};
pub use error::{
    Error, ErrorContext, ErrorKind, ErrorResponse, FieldError, Result, RichError,
    ValidationErrorBuilder,
};
pub use events::{
    BackpressureConfig, BackpressureController, BackpressureError, BackpressureMetrics,
    BackpressureState, DispatcherConfig, DomainEvent, EVENT_SCHEMAS, EventBus, EventBusStats,
    EventConsumerRuntime, EventDispatcher, EventEnvelope, EventHandler, EventSchema,
    EventTransport, FieldSchema, HandlerBuilder, HandlerResult, MemoryTransport, ReliabilityLevel,
    RunningDispatcher, event_schema,
};
pub use field_schema::{
    CustomFieldsSchema, FieldDefinition, FieldErrorCode, FieldType, FieldValidationError,
    FlexError, HasCustomFields, MAX_JSON_NESTING_DEPTH, SelectOption, ValidationRule,
    create_field_definitions_table, drop_field_definitions_table, is_valid_field_key,
    is_valid_locale_key, json_field_contains, json_field_eq, json_field_exists, json_field_extract,
    json_object_depth,
};
pub use health::{
    HealthCheck, HealthRegistry, HealthResult, HealthStatus, OverallHealth,
    checks::{DatabaseHealthCheck, FnHealthCheck},
};
pub use i18n::{Locale, extract_locale_from_header, translate};
pub use id::{generate_id, parse_id};
pub use metrics::{Counter, Gauge, Histogram, MetricSnapshot, MetricValue, MetricsRegistry, Timer};
pub use migrations::{
    MigrationDependencyDescriptor, MigrationPhaseConstraint, MigrationSafetyClass,
    MigrationSafetyMetadata, ModuleMigration,
};
pub use module::{
    MigrationSource, MigrationTrait, ModuleContext, ModuleEventListenerContext,
    ModuleEventListenerRegistry, ModuleKind, ModuleRuntimeExtensions, RusToKModule,
};
pub use rbac::{
    PermissionScope, Rbac, SecurityActorKind, SecurityContext, infer_user_role_from_permissions,
};
pub use registry::ModuleRegistry;
pub use resilience::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitState, RetryPolicy,
    RetryStrategy,
};
pub use retention::{RetentionPolicy, RetentionPolicyError};
pub use security::{
    AuditEvent, AuditLogger, InputValidator, RateLimitConfig, RateLimitResult, RateLimiter,
    SecurityAudit, SecurityAuditResult, SecurityCategory, SecurityConfig, SecurityFinding,
    SecurityHeaders, SecurityHeadersConfig, Severity, SsrfProtection, ValidationResult,
    audit::AuditEventType, headers::FrameOptions, run_security_audit,
};
pub use security_principal::security_context_from_access_token;
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
    pub use crate::async_utils::{BackoffConfig, RetryError, Throttler, batch, parallel, retry};
    pub use crate::config::{ConfigLoader, ConfigSource, Secret};
    pub use crate::domain_err;
    pub use crate::error::{Error, Result};
    pub use crate::events::{
        BackpressureConfig, BackpressureController, BackpressureError, BackpressureMetrics,
        BackpressureState, DispatcherConfig, DomainEvent, EVENT_SCHEMAS, EventBus, EventBusStats,
        EventConsumerRuntime, EventDispatcher, EventEnvelope, EventHandler, EventSchema,
        EventTransport, FieldSchema, HandlerBuilder, HandlerResult, MemoryTransport,
        ReliabilityLevel, RunningDispatcher, event_schema,
    };
    pub use crate::field_schema::{
        CustomFieldsSchema, FieldDefinition, FieldType, HasCustomFields,
    };
    pub use crate::health::{
        HealthCheck, HealthRegistry, HealthResult, HealthStatus, OverallHealth,
    };
    pub use crate::id::generate_id;
    pub use crate::metrics::{Counter, Gauge, Histogram, MetricsRegistry, Timer};
    pub use crate::rbac::{PermissionScope, Rbac, SecurityActorKind, SecurityContext};
    pub use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
    pub use crate::security_principal::security_context_from_access_token;
    pub use crate::typed_error::{DomainError, ErrorCode, TypedResult};
    pub use crate::types::{UserRole, UserStatus};
    pub use crate::{
        AppContext, CacheBackend, CacheCompareAndSetOutcome, CacheStats, InMemoryCacheBackend,
        SearchBackend,
    };
    pub use rustok_api::{Action, Permission, Resource};
    pub use uuid::Uuid;
}

#[cfg(test)]
mod contract_tests;
