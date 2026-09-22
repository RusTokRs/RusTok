use crate::adapters::FlyAdapterBackedPageBuilderService;
use crate::preview_port::PageBuilderPreviewRenderingPort;
use crate::rollout::{BuilderCapabilityFlags, BuilderRolloutError};
use crate::runtime_scenario_release::{
    NoopPageBuilderScenarioBaselineStore, PageBuilderScenarioBaselineStore,
};
use crate::runtime_telemetry::{PageBuilderRuntimeTelemetry, ProviderHealthRuntimeTelemetry};
use crate::service::{
    AuthorizedPageBuilderHandlers, CapabilityGuardedService, PageBuilderCapabilityAuthorizer,
    PageBuilderCapabilityPortPolicies, PageBuilderProjectStore,
};

pub type FlyPageBuilderGuardedService<
    S,
    R,
    T = ProviderHealthRuntimeTelemetry,
    B = NoopPageBuilderScenarioBaselineStore,
> = CapabilityGuardedService<FlyAdapterBackedPageBuilderService<S, R, T, B>>;

pub type FlyPageBuilderHandlers<
    S,
    R,
    T = ProviderHealthRuntimeTelemetry,
    B = NoopPageBuilderScenarioBaselineStore,
> = AuthorizedPageBuilderHandlers<FlyPageBuilderGuardedService<S, R, T, B>>;

/// Compose the default current-only Page Builder server pipeline.
///
/// Consumers provide tenant-scoped persistence and contextual preview rendering ports. The module
/// owns the service, rollout/port guards and authorization order. The default pipeline also records
/// bounded process-local Preview/Publish runtime observations; the health window remains
/// `unobserved` until its minimum sample floor is satisfied and is not deployment-wide evidence.
pub fn compose_fly_page_builder_handlers<S, R>(
    store: S,
    renderer: R,
    flags: BuilderCapabilityFlags,
) -> Result<FlyPageBuilderHandlers<S, R>, BuilderRolloutError>
where
    S: PageBuilderProjectStore,
    R: PageBuilderPreviewRenderingPort,
{
    let service = FlyAdapterBackedPageBuilderService::with_telemetry(
        store,
        renderer,
        ProviderHealthRuntimeTelemetry::default(),
    );
    compose_configured_fly_page_builder_handlers(
        service,
        flags,
        PageBuilderCapabilityPortPolicies::default(),
        PageBuilderCapabilityAuthorizer::default(),
    )
}

/// Compose the current-only Page Builder server pipeline with explicit policies and authorizer.
///
/// Telemetry, scenario baselines and Fly validation policy are configured on `service` before it is
/// passed here. Invalid rollout flags are rejected before handlers can be exposed. Explicitly
/// configured services may still supply another telemetry implementation when a host owns a
/// different observation sink.
pub fn compose_configured_fly_page_builder_handlers<S, R, T, B>(
    service: FlyAdapterBackedPageBuilderService<S, R, T, B>,
    flags: BuilderCapabilityFlags,
    policies: PageBuilderCapabilityPortPolicies,
    authorizer: PageBuilderCapabilityAuthorizer,
) -> Result<FlyPageBuilderHandlers<S, R, T, B>, BuilderRolloutError>
where
    S: PageBuilderProjectStore,
    R: PageBuilderPreviewRenderingPort,
    T: PageBuilderRuntimeTelemetry,
    B: PageBuilderScenarioBaselineStore,
{
    flags.validate()?;
    let guarded = CapabilityGuardedService::with_policies(service, flags, policies);
    Ok(AuthorizedPageBuilderHandlers::with_authorizer(
        guarded, authorizer,
    ))
}
