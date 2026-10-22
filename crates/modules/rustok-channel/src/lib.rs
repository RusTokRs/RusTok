//! # RusToK Channel Module
//!
//! Platform-level channel context for external delivery surfaces such as websites,
//! mobile applications, API clients, and embedded targets.
//!
//! Provides channel entity management, target mappings, module bindings, OAuth app associations,
//! request-level channel resolution pipelines, cache invalidation generations, and transport-neutral
//! channel read ports.

pub mod dto;
pub mod entities;
pub mod error;
pub mod invalidation_generation;
pub mod migrations;
pub mod module;
pub mod policy;
pub mod ports;
pub mod resolution;
pub mod services;
pub mod target_type;

#[cfg(test)]
mod contract_tests;

pub use dto::{
    AvailableChannelModuleItem, AvailableChannelOauthAppItem, BindChannelModuleInput,
    BindChannelOauthAppInput, ChannelBootstrapResponse, ChannelDetailResponse,
    ChannelModuleBindingResponse, ChannelOauthAppResponse,
    ChannelResolutionPolicySetDetailResponse, ChannelResolutionPolicySetResponse,
    ChannelResolutionRuleResponse, ChannelResponse, ChannelTargetResponse, CreateChannelInput,
    CreateChannelRequest, CreateChannelResolutionPolicySetInput, CreateChannelResolutionRuleInput,
    CreateChannelTargetInput, CreateResolutionPolicySetRequest, CreateResolutionRuleRequest,
    ReorderChannelResolutionRulesInput, ReorderResolutionRulesRequest,
    UpdateChannelResolutionRuleInput, UpdateChannelTargetInput, UpdateResolutionRuleRequest,
    create_channel_input, create_resolution_policy_set_input, create_resolution_rule_input,
    update_resolution_rule_input,
};
pub use error::{ChannelError, ChannelResult};
pub use invalidation_generation::{
    CHANNEL_RESOLUTION_INVALIDATION_SCOPE, read_resolution_invalidation_generation,
};
pub use module::{ChannelModule, ChannelRuntimeSelected};
pub use policy::{
    CHANNEL_RESOLUTION_POLICY_SCHEMA_VERSION, ChannelResolutionRuleDefinition, ResolutionAction,
    ResolutionPredicate, StoredChannelResolutionRule,
};
pub use ports::{
    ChannelListProjectionPage, ChannelListRequest, ChannelReadPort, ChannelReadProjection,
    ChannelReadRequest, ChannelReadSelector, PortActor, PortCallPolicy, PortContext, PortError,
    PortErrorKind, in_process_channel_read_port,
};
pub use resolution::{
    ChannelResolutionOrigin, ChannelResolver, RequestFacts, ResolutionDecision, ResolutionOutcome,
    ResolutionStage, ResolutionTraceStep, TargetSurface,
};
pub use services::ChannelService;
pub use target_type::ChannelTargetType;
