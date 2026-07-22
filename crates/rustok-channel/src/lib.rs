pub mod dto;
pub mod entities;
pub mod error;
pub mod invalidation_generation;
pub mod migrations;
pub mod policy;
pub mod ports;
pub mod resolution;
pub mod services;
pub mod target_type;

pub use dto::{
    AvailableChannelModuleItem, AvailableChannelOauthAppItem, BindChannelModuleInput,
    BindChannelOauthAppInput, ChannelBootstrapResponse, ChannelDetailResponse,
    ChannelModuleBindingResponse, ChannelOauthAppResponse,
    ChannelResolutionPolicySetDetailResponse, ChannelResolutionPolicySetResponse,
    ChannelResolutionRuleResponse, ChannelResponse, ChannelTargetResponse, CreateChannelInput,
    CreateChannelResolutionPolicySetInput, CreateChannelResolutionRuleInput,
    CreateChannelTargetInput, CreateResolutionPolicySetRequest, CreateResolutionRuleRequest,
    ReorderChannelResolutionRulesInput, ReorderResolutionRulesRequest,
    UpdateChannelResolutionRuleInput, UpdateChannelTargetInput, UpdateResolutionRuleRequest,
    create_resolution_policy_set_input, create_resolution_rule_input, update_resolution_rule_input,
};
pub use error::{ChannelError, ChannelResult};
pub use invalidation_generation::{
    CHANNEL_RESOLUTION_INVALIDATION_SCOPE, read_resolution_invalidation_generation,
};
pub use policy::{
    CHANNEL_RESOLUTION_POLICY_SCHEMA_VERSION, ChannelResolutionRuleDefinition, ResolutionAction,
    ResolutionPredicate, StoredChannelResolutionRule,
};
pub use ports::*;
pub use resolution::{
    ChannelResolutionOrigin, ChannelResolver, RequestFacts, ResolutionDecision, ResolutionOutcome,
    ResolutionStage, ResolutionTraceStep, TargetSurface,
};
pub use services::ChannelService;
pub use target_type::ChannelTargetType;

use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub struct ChannelModule;

impl MigrationSource for ChannelModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[async_trait]
impl RusToKModule for ChannelModule {
    fn slug(&self) -> &'static str {
        "channel"
    }

    fn name(&self) -> &'static str {
        "Channel"
    }

    fn description(&self) -> &'static str {
        "Experimental core channel-management context for external delivery surfaces."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
