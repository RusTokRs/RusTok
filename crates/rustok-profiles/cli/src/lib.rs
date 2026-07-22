//! External operational command adapters for `rustok-profiles`.

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_auth::{
    AuthUserBackfillDbReader, AuthUserBackfillReadPort, AuthUserBackfillReadRequest,
};
use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_core::events::EventTransport;
use rustok_customer::CustomerService;
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_profiles::{ProfileService, ProfileVisibility, ProfilesReader};
use rustok_runtime::{RuntimeComposition, db_clone};
use rustok_tenant::{TenantReadPort, TenantReadRequest, TenantReadSelector, TenantService};
use uuid::Uuid;

pub struct ProfilesCommandProvider {
    runtime: RuntimeComposition,
}

#[async_trait::async_trait]
impl CommandProvider for ProfilesCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new(
                "profiles",
                "backfill",
                "Backfill missing public profiles for tenant users",
            )
            .with_dry_run(),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("profiles", "backfill") => self.backfill(request).await,
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

impl ProfilesCommandProvider {
    async fn backfill(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        let options = options(&request.args)?;
        let tenant_id = required_uuid(options, "tenant_id")?;
        let limit = optional_u64(options, "limit")?.unwrap_or(500);
        let visibility = visibility(options)?;
        let dry_run = request.dry_run || flag(options, "dry_run");
        let emit_events = flag(options, "emit_events");
        let host = self
            .runtime
            .require_host()
            .map_err(|error| command_failed(error.to_string()))?;
        let db = db_clone(host);
        let context = port_context(tenant_id);
        let tenant = TenantService::new(db.clone())
            .read_tenant(
                context.clone(),
                TenantReadRequest {
                    selector: TenantReadSelector::Id(tenant_id),
                    include_inactive: true,
                },
            )
            .await
            .map_err(|error| command_failed(error.message))?;
        let users = AuthUserBackfillDbReader::new(db.clone())
            .list_users_for_profile_backfill(AuthUserBackfillReadRequest { tenant_id, limit })
            .await
            .map_err(command_failed)?;
        let enrichments = CustomerService::new(db.clone())
            .list_profile_enrichment(
                tenant_id,
                &users.iter().map(|user| user.id).collect::<Vec<_>>(),
            )
            .await
            .map_err(command_failed)?
            .into_iter()
            .map(|item| (item.user_id, item))
            .collect::<HashMap<_, _>>();
        let event_bus = (!dry_run && emit_events).then(|| {
            TransactionalEventBus::new(
                Arc::new(OutboxTransport::new(db.clone())) as Arc<dyn EventTransport>
            )
        });
        let profiles = ProfileService::new(db);
        let existing = profiles
            .find_profile_summaries(
                tenant.id,
                &users.iter().map(|user| user.id).collect::<Vec<_>>(),
                Some(&tenant.default_locale),
                Some(&tenant.default_locale),
            )
            .await
            .map_err(command_failed)?;
        let scanned_users = users.len();
        let mut items = Vec::new();
        let mut skipped_existing = 0usize;
        let mut planned_creates = 0usize;
        let mut created_profiles = 0usize;
        let mut published_events = 0usize;
        for user in users {
            if existing.contains_key(&user.id) {
                skipped_existing += 1;
                continue;
            }
            let enrichment = enrichments.get(&user.id);
            let customer_display_name = enrichment.and_then(display_name);
            let display_name = customer_display_name.as_deref().or(user.name.as_deref());
            let locale = enrichment
                .and_then(|item| item.preferred_locale.as_deref())
                .unwrap_or(&tenant.default_locale);
            let plan = profiles
                .plan_backfill_profile(
                    tenant.id,
                    user.id,
                    &user.email,
                    display_name,
                    Some(locale),
                    visibility,
                )
                .await
                .map_err(command_failed)?;
            if dry_run {
                planned_creates += 1;
                items.push(serde_json::json!({"user_id": user.id, "email": user.email, "handle": plan.handle, "display_name": plan.display_name, "preferred_locale": plan.preferred_locale, "action": "planned", "event_published": false}));
                continue;
            }
            let result = profiles
                .backfill_profile(
                    tenant.id,
                    user.id,
                    &user.email,
                    display_name,
                    Some(locale),
                    visibility,
                    Some(&tenant.default_locale),
                )
                .await
                .map_err(command_failed)?;
            let mut event_published = false;
            if result.created {
                created_profiles += 1;
                if let Some(bus) = &event_bus {
                    bus.publish(
                        tenant.id,
                        None,
                        DomainEvent::ProfileUpdated {
                            user_id: result.profile.user_id,
                            handle: result.profile.handle.clone(),
                            locale: result.profile.preferred_locale.clone(),
                        },
                    )
                    .await
                    .map_err(command_failed)?;
                    published_events += 1;
                    event_published = true;
                }
            } else {
                skipped_existing += 1;
            }
            items.push(serde_json::json!({"user_id": user.id, "email": user.email, "handle": result.profile.handle, "display_name": result.profile.display_name, "preferred_locale": result.profile.preferred_locale, "action": if result.created { "created" } else { "skipped" }, "event_published": event_published}));
        }
        Ok(CommandOutcome::success("Profiles backfill complete").with_data(serde_json::json!({"generated_at": Utc::now().to_rfc3339(), "tenant_id": tenant.id, "tenant_slug": tenant.slug, "tenant_default_locale": tenant.default_locale, "dry_run": dry_run, "emit_events": emit_events, "visibility": visibility.to_string(), "limit": limit, "scanned_users": scanned_users, "skipped_existing": skipped_existing, "planned_creates": planned_creates, "created_profiles": created_profiles, "published_events": published_events, "items": items})))
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(ProfilesCommandProvider {
        runtime: runtime.clone(),
    })
}

fn options(args: &serde_json::Value) -> CliCoreResult<&serde_json::Map<String, serde_json::Value>> {
    args.get("options")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: "profiles backfill expects normalized command options".to_string(),
        })
}
fn required_uuid(
    options: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> CliCoreResult<Uuid> {
    options
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: format!("--{key} is required"),
        })
        .and_then(|raw| {
            Uuid::parse_str(raw).map_err(|_| CliCoreError::InvalidInput {
                message: format!("--{key} must be a UUID"),
            })
        })
}
fn optional_u64(
    options: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> CliCoreResult<Option<u64>> {
    options
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| CliCoreError::InvalidInput {
                    message: format!("--{key} must be a positive integer"),
                })
                .and_then(|raw| {
                    raw.parse().map_err(|_| CliCoreError::InvalidInput {
                        message: format!("--{key} must be a positive integer"),
                    })
                })
        })
        .transpose()
}
fn flag(options: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    options
        .get(key)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| matches!(value, "1" | "true" | "yes" | "on"))
}
fn visibility(
    options: &serde_json::Map<String, serde_json::Value>,
) -> CliCoreResult<ProfileVisibility> {
    match options
        .get("visibility")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("authenticated")
    {
        "public" => Ok(ProfileVisibility::Public),
        "authenticated" => Ok(ProfileVisibility::Authenticated),
        "followers_only" => Ok(ProfileVisibility::FollowersOnly),
        "private" => Ok(ProfileVisibility::Private),
        _ => Err(CliCoreError::InvalidInput {
            message: "--visibility must be public|authenticated|followers_only|private".to_string(),
        }),
    }
}
fn port_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "profiles-backfill",
    )
    .with_deadline(Duration::from_secs(5))
}
fn display_name(item: &rustok_customer::CustomerProfileEnrichment) -> Option<String> {
    let first = item
        .first_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let last = item
        .last_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (first, last) {
        (Some(first), Some(last)) => Some(format!("{first} {last}")),
        (Some(first), None) => Some(first.to_string()),
        (None, Some(last)) => Some(last.to_string()),
        (None, None) => None,
    }
}
fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}
