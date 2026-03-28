pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod policy;
pub mod resolution;
pub mod services;
pub mod target_type;

pub use dto::{
    BindChannelModuleInput, BindChannelOauthAppInput, ChannelDetailResponse,
    ChannelModuleBindingResponse, ChannelOauthAppResponse,
    ChannelResolutionPolicySetDetailResponse, ChannelResolutionPolicySetResponse,
    ChannelResolutionRuleResponse, ChannelResponse, ChannelTargetResponse, CreateChannelInput,
    CreateChannelResolutionPolicySetInput, CreateChannelResolutionRuleInput,
    CreateChannelTargetInput, UpdateChannelTargetInput,
};
pub use error::{ChannelError, ChannelResult};
pub use policy::{
    CHANNEL_RESOLUTION_POLICY_SCHEMA_VERSION, ChannelResolutionRuleDefinition, ResolutionAction,
    ResolutionPredicate, StoredChannelResolutionRule,
};
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
