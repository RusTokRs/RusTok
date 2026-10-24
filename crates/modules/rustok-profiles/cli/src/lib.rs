//! External operational command adapters for `rustok-profiles`.

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_auth::{
    AuthUserBackfillDbReader, AuthUserBackfillReadPort, AuthUserBackfillReadRequest,
};
use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_core::events::EventTransport;
use rustok_customer::CustomerService;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_profiles::{
    ProfileBackfillRequest, ProfileBackfillTimer, ProfileError, ProfileMutationService,
    ProfileService, ProfileVisibility, ProfilesReader,
};
use rustok_runtime::{RuntimeComposition, db_clone};
use rustok_tenant::{TenantReadPort, TenantReadRequest, TenantReadSelector, TenantService};
use uuid::Uuid;

const BACKFILL_HOST_ERROR: &str = "profiles.backfill_host_unavailable";
const BACKFILL_TENANT_READ_ERROR: &str = "profiles.backfill_tenant_read_failed";
const BACKFILL_USER_READ_ERROR: &str = "profiles.backfill_user_read_failed";
const BACKFILL_ENRICHMENT_READ_ERROR: &str = "profiles.backfill_enrichment_read_failed";

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
        let emit_events = !dry_run;
        let telemetry = ProfileBackfillTimer::start(tenant_id, dry_run, emit_events);
        let host = self.runtime.require_host().map_err(|error| {
            backfill_failed(
                &telemetry,
                "host_resolution",
                BACKFILL_HOST_ERROR,
                false,
                error,
            )
        })?;
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
            .map_err(|error| {
                backfill_port_failed(&telemetry, "tenant_read", BACKFILL_TENANT_READ_ERROR, error)
            })?;
        let users = AuthUserBackfillDbReader::new(db.clone())
            .list_users_for_profile_backfill(AuthUserBackfillReadRequest { tenant_id, limit })
            .await
            .map_err(|error| {
                backfill_failed(
                    &telemetry,
                    "user_read",
                    BACKFILL_USER_READ_ERROR,
                    true,
                    error,
                )
            })?;
        let enrichments = CustomerService::new(db.clone())
            .list_profile_enrichment(
                tenant_id,
                &users.iter().map(|user| user.id).collect::<Vec<_>>(),
            )
            .await
            .map_err(|error| {
                backfill_failed(
                    &telemetry,
                    "enrichment_read",
                    BACKFILL_ENRICHMENT_READ_ERROR,
                    true,
                    error,
                )
            })?
            .into_iter()
            .map(|item| (item.user_id, item))
            .collect::<HashMap<_, _>>();
        let event_bus = TransactionalEventBus::new(
            Arc::new(OutboxTransport::new(db.clone())) as Arc<dyn EventTransport>
        );
        let profile_mutations = ProfileMutationService::new(&db, &event_bus);
        let profiles = ProfileService::new(db.clone());
        let existing = profiles
            .find_profile_summaries(
                tenant.id,
                &users.iter().map(|user| user.id).collect::<Vec<_>>(),
                Some(&tenant.default_locale),
                Some(&tenant.default_locale),
            )
            .await
            .map_err(|error| backfill_profile_failed(&telemetry, "existing_profile_read", error))?;
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
            if dry_run {
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
                    .map_err(|error| backfill_profile_failed(&telemetry, "profile_plan", error))?;
                planned_creates += 1;
                items.push(serde_json::json!({"user_id": user.id, "email": user.email, "handle": plan.handle, "display_name": plan.display_name, "preferred_locale": plan.preferred_locale, "action": "planned", "event_published": false}));
                continue;
            }
            let result = profile_mutations
                .backfill_profile_with_event(ProfileBackfillRequest {
                    tenant_id: tenant.id,
                    user_id: user.id,
                    email: &user.email,
                    display_name,
                    preferred_locale: Some(locale),
                    visibility,
                    tenant_default_locale: Some(&tenant.default_locale),
                })
                .await
                .map_err(|error| backfill_profile_failed(&telemetry, "profile_create", error))?;
            let event_published = result.created;
            if result.created {
                created_profiles += 1;
                published_events += 1;
            } else {
                skipped_existing += 1;
            }
            items.push(serde_json::json!({"user_id": user.id, "email": user.email, "handle": result.profile.handle, "display_name": result.profile.display_name, "preferred_locale": result.profile.preferred_locale, "action": if result.created { "created" } else { "skipped" }, "event_published": event_published}));
        }
        telemetry.finish_success(
            scanned_users,
            skipped_existing,
            planned_creates,
            created_profiles,
            published_events,
        );
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

fn backfill_port_failed(
    telemetry: &ProfileBackfillTimer,
    stage: &'static str,
    error_code: &'static str,
    error: PortError,
) -> CliCoreError {
    telemetry.finish_failure(stage, error_code, error.retryable);
    command_failed(error.message)
}

fn backfill_profile_failed(
    telemetry: &ProfileBackfillTimer,
    stage: &'static str,
    error: ProfileError,
) -> CliCoreError {
    telemetry.finish_failure(stage, error.code(), error.is_retryable());
    command_failed(error)
}

fn backfill_failed(
    telemetry: &ProfileBackfillTimer,
    stage: &'static str,
    error_code: &'static str,
    retryable: bool,
    error: impl std::fmt::Display,
) -> CliCoreError {
    telemetry.finish_failure(stage, error_code, retryable);
    command_failed(error)
}

fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_customer::CustomerProfileEnrichment;
    use serde_json::json;

    #[test]
    fn test_command_descriptor() {
        let provider = ProfilesCommandProvider {
            runtime: RuntimeComposition::without_database(json!({})),
        };
        let commands = provider.commands();
        assert_eq!(commands.len(), 1);
        let cmd = &commands[0];
        assert_eq!(cmd.namespace, "profiles");
        assert_eq!(cmd.name, "backfill");
        assert!(cmd.supports_dry_run);
    }

    #[test]
    fn test_flag_parsing() {
        let map: serde_json::Map<String, serde_json::Value> = [
            ("enabled_true".to_string(), json!("true")),
            ("enabled_1".to_string(), json!("1")),
            ("enabled_yes".to_string(), json!("yes")),
            ("enabled_on".to_string(), json!("on")),
            ("disabled_false".to_string(), json!("false")),
            ("disabled_0".to_string(), json!("0")),
        ]
        .into_iter()
        .collect();

        assert!(flag(&map, "enabled_true"));
        assert!(flag(&map, "enabled_1"));
        assert!(flag(&map, "enabled_yes"));
        assert!(flag(&map, "enabled_on"));
        assert!(!flag(&map, "disabled_false"));
        assert!(!flag(&map, "disabled_0"));
        assert!(!flag(&map, "nonexistent"));
    }

    #[test]
    fn test_visibility_parsing() {
        let mut map = serde_json::Map::new();
        assert_eq!(visibility(&map).unwrap(), ProfileVisibility::Authenticated);

        map.insert("visibility".to_string(), json!("public"));
        assert_eq!(visibility(&map).unwrap(), ProfileVisibility::Public);

        map.insert("visibility".to_string(), json!("followers_only"));
        assert_eq!(visibility(&map).unwrap(), ProfileVisibility::FollowersOnly);

        map.insert("visibility".to_string(), json!("private"));
        assert_eq!(visibility(&map).unwrap(), ProfileVisibility::Private);

        map.insert("visibility".to_string(), json!("invalid"));
        assert!(visibility(&map).is_err());
    }

    #[test]
    fn test_required_uuid_parsing() {
        let mut map = serde_json::Map::new();
        assert!(required_uuid(&map, "tenant_id").is_err());

        map.insert("tenant_id".to_string(), json!("not-a-uuid"));
        assert!(required_uuid(&map, "tenant_id").is_err());

        let id = Uuid::new_v4();
        map.insert("tenant_id".to_string(), json!(id.to_string()));
        assert_eq!(required_uuid(&map, "tenant_id").unwrap(), id);
    }

    #[test]
    fn test_optional_u64_parsing() {
        let mut map = serde_json::Map::new();
        assert_eq!(optional_u64(&map, "limit").unwrap(), None);

        map.insert("limit".to_string(), json!("100"));
        assert_eq!(optional_u64(&map, "limit").unwrap(), Some(100));

        map.insert("limit".to_string(), json!("not-a-number"));
        assert!(optional_u64(&map, "limit").is_err());
    }

    #[test]
    fn test_display_name_formatting() {
        let item = CustomerProfileEnrichment {
            user_id: Uuid::new_v4(),
            first_name: Some(" Jane ".to_string()),
            last_name: Some(" Doe ".to_string()),
            preferred_locale: Some("en".to_string()),
        };
        assert_eq!(display_name(&item), Some("Jane Doe".to_string()));

        let item_first_only = CustomerProfileEnrichment {
            user_id: Uuid::new_v4(),
            first_name: Some("Jane".to_string()),
            last_name: None,
            preferred_locale: None,
        };
        assert_eq!(display_name(&item_first_only), Some("Jane".to_string()));

        let item_last_only = CustomerProfileEnrichment {
            user_id: Uuid::new_v4(),
            first_name: None,
            last_name: Some("Doe".to_string()),
            preferred_locale: None,
        };
        assert_eq!(display_name(&item_last_only), Some("Doe".to_string()));

        let item_empty = CustomerProfileEnrichment {
            user_id: Uuid::new_v4(),
            first_name: Some("   ".to_string()),
            last_name: None,
            preferred_locale: None,
        };
        assert_eq!(display_name(&item_empty), None);
    }
}
