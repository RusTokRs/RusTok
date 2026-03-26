use async_trait::async_trait;
use chrono::Utc;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};
use rustok_events::DomainEvent;
use rustok_profiles::{ProfileService, ProfileVisibility, ProfilesReader};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use uuid::Uuid;

use crate::models::{tenants, users};
use crate::services::event_bus::transactional_event_bus_from_context;

#[cfg(feature = "mod-customer")]
use rustok_customer::entities::customer;

pub struct ProfilesBackfillTask;

#[async_trait]
impl Task for ProfilesBackfillTask {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "profiles_backfill".to_string(),
            detail:
                "Backfill missing profiles for existing users with dry-run and optional profile.updated publishing"
                    .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let tenant = resolve_tenant(ctx, vars).await?;
        let dry_run = is_flag_enabled(vars, "dry_run");
        let emit_events = is_flag_enabled(vars, "emit_events");
        let limit = parse_limit(vars)?;
        let visibility = parse_visibility(vars)?;

        let users = users::Entity::find()
            .filter(users::Column::TenantId.eq(tenant.id))
            .order_by_asc(users::Column::CreatedAt)
            .limit(limit)
            .all(&ctx.db)
            .await
            .map_err(|error| Error::Message(format!("Failed to load users: {error}")))?;

        let user_ids = users.iter().map(|user| user.id).collect::<Vec<_>>();
        let profile_service = ProfileService::new(ctx.db.clone());
        let existing_profiles = profile_service
            .find_profile_summaries(
                tenant.id,
                &user_ids,
                Some(tenant.default_locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|error| {
                Error::Message(format!("Failed to load existing profiles: {error}"))
            })?;
        let customer_profiles = load_customer_map(ctx, tenant.id, &user_ids).await?;

        let event_bus =
            (!dry_run && emit_events).then(|| transactional_event_bus_from_context(ctx));
        let mut report = ProfilesBackfillReport {
            generated_at: Utc::now().to_rfc3339(),
            tenant_id: tenant.id,
            tenant_slug: tenant.slug,
            tenant_default_locale: tenant.default_locale.clone(),
            dry_run,
            emit_events,
            visibility: visibility.to_string(),
            limit,
            scanned_users: users.len(),
            skipped_existing: 0,
            planned_creates: 0,
            created_profiles: 0,
            published_events: 0,
            items: Vec::new(),
        };

        for user in users {
            if existing_profiles.contains_key(&user.id) {
                report.skipped_existing += 1;
                continue;
            }

            let customer = customer_profiles.get(&user.id);
            let customer_display_name = customer_display_name(customer);
            let display_name = customer_display_name.as_deref().or(user.name.as_deref());
            let customer_locale = customer_preferred_locale(customer);
            let preferred_locale = customer_locale
                .as_deref()
                .or(Some(tenant.default_locale.as_str()));
            let plan = profile_service
                .plan_backfill_profile(
                    tenant.id,
                    user.id,
                    &user.email,
                    display_name,
                    preferred_locale,
                    visibility,
                )
                .await
                .map_err(|error| {
                    Error::Message(format!(
                        "Failed to plan profile backfill for user {}: {error}",
                        user.id
                    ))
                })?;

            if dry_run {
                report.planned_creates += 1;
                report.items.push(ProfilesBackfillItem {
                    user_id: user.id,
                    email: user.email,
                    handle: plan.handle,
                    display_name: plan.display_name,
                    preferred_locale: plan.preferred_locale,
                    action: "planned".to_string(),
                    event_published: false,
                });
                continue;
            }

            let result = profile_service
                .backfill_profile(
                    tenant.id,
                    user.id,
                    &user.email,
                    display_name,
                    preferred_locale,
                    visibility,
                    Some(tenant.default_locale.as_str()),
                )
                .await
                .map_err(|error| {
                    Error::Message(format!(
                        "Failed to backfill profile for user {}: {error}",
                        user.id
                    ))
                })?;

            let mut event_published = false;
            if result.created {
                report.created_profiles += 1;
                if let Some(event_bus) = &event_bus {
                    event_bus
                        .publish(
                            tenant.id,
                            None,
                            DomainEvent::ProfileUpdated {
                                user_id: result.profile.user_id,
                                handle: result.profile.handle.clone(),
                                locale: result.profile.preferred_locale.clone(),
                            },
                        )
                        .await
                        .map_err(|error| {
                            Error::Message(format!(
                                "Failed to publish profile.updated for user {}: {error}",
                                result.profile.user_id
                            ))
                        })?;
                    report.published_events += 1;
                    event_published = true;
                }
            } else {
                report.skipped_existing += 1;
            }

            report.items.push(ProfilesBackfillItem {
                user_id: user.id,
                email: user.email,
                handle: result.profile.handle,
                display_name: result.profile.display_name,
                preferred_locale: result.profile.preferred_locale,
                action: if result.created {
                    "created".to_string()
                } else {
                    "skipped".to_string()
                },
                event_published,
            });
        }

        let payload = serde_json::to_string_pretty(&report).map_err(|error| {
            Error::Message(format!(
                "Failed to serialize profiles backfill report: {error}"
            ))
        })?;
        println!("{payload}");
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct ProfilesBackfillReport {
    generated_at: String,
    tenant_id: Uuid,
    tenant_slug: String,
    tenant_default_locale: String,
    dry_run: bool,
    emit_events: bool,
    visibility: String,
    limit: u64,
    scanned_users: usize,
    skipped_existing: usize,
    planned_creates: usize,
    created_profiles: usize,
    published_events: usize,
    items: Vec<ProfilesBackfillItem>,
}

#[derive(Debug, Serialize)]
struct ProfilesBackfillItem {
    user_id: Uuid,
    email: String,
    handle: String,
    display_name: String,
    preferred_locale: Option<String>,
    action: String,
    event_published: bool,
}

async fn resolve_tenant(
    ctx: &AppContext,
    vars: &Vars,
) -> Result<crate::models::_entities::tenants::Model> {
    let Some(raw_tenant_id) = vars.cli.get("tenant_id") else {
        return Err(Error::Message(
            "profiles_backfill requires tenant_id=<uuid>".to_string(),
        ));
    };
    let tenant_id = Uuid::parse_str(raw_tenant_id)
        .map_err(|error| Error::Message(format!("Invalid tenant_id '{raw_tenant_id}': {error}")))?;

    tenants::Entity::find_by_id(&ctx.db, tenant_id)
        .await
        .map_err(|error| Error::Message(format!("Failed to load tenant {tenant_id}: {error}")))?
        .ok_or_else(|| Error::Message(format!("Tenant {tenant_id} not found")))
}

fn parse_limit(vars: &Vars) -> Result<u64> {
    match vars.cli.get("limit") {
        Some(raw) => raw
            .parse::<u64>()
            .map_err(|error| Error::Message(format!("Invalid limit '{raw}': {error}"))),
        None => Ok(500),
    }
}

fn parse_visibility(vars: &Vars) -> Result<ProfileVisibility> {
    match vars
        .cli
        .get("visibility")
        .map(String::as_str)
        .unwrap_or("authenticated")
    {
        "public" => Ok(ProfileVisibility::Public),
        "authenticated" => Ok(ProfileVisibility::Authenticated),
        "followers_only" => Ok(ProfileVisibility::FollowersOnly),
        "private" => Ok(ProfileVisibility::Private),
        other => Err(Error::Message(format!(
            "Invalid visibility '{other}', expected public|authenticated|followers_only|private"
        ))),
    }
}

fn is_flag_enabled(vars: &Vars, key: &str) -> bool {
    vars.cli
        .get(key)
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

#[cfg(feature = "mod-customer")]
async fn load_customer_map(
    ctx: &AppContext,
    tenant_id: Uuid,
    user_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, customer::Model>> {
    let customers = customer::Entity::find()
        .filter(customer::Column::TenantId.eq(tenant_id))
        .filter(customer::Column::UserId.is_in(user_ids.iter().copied()))
        .all(&ctx.db)
        .await
        .map_err(|error| Error::Message(format!("Failed to load customers: {error}")))?;

    Ok(customers
        .into_iter()
        .filter_map(|customer| customer.user_id.map(|user_id| (user_id, customer)))
        .collect())
}

#[cfg(not(feature = "mod-customer"))]
async fn load_customer_map(
    _ctx: &AppContext,
    _tenant_id: Uuid,
    _user_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, ()>> {
    Ok(std::collections::HashMap::new())
}

#[cfg(feature = "mod-customer")]
fn customer_display_name(customer: Option<&customer::Model>) -> Option<String> {
    let customer = customer?;
    match (
        customer.first_name.as_deref(),
        customer.last_name.as_deref(),
    ) {
        (Some(first_name), Some(last_name))
            if !first_name.trim().is_empty() && !last_name.trim().is_empty() =>
        {
            Some(format!("{} {}", first_name.trim(), last_name.trim()))
        }
        (Some(first_name), _) if !first_name.trim().is_empty() => {
            Some(first_name.trim().to_string())
        }
        (_, Some(last_name)) if !last_name.trim().is_empty() => Some(last_name.trim().to_string()),
        _ => None,
    }
}

#[cfg(not(feature = "mod-customer"))]
fn customer_display_name(_customer: Option<&()>) -> Option<String> {
    None
}

#[cfg(feature = "mod-customer")]
fn customer_preferred_locale(customer: Option<&customer::Model>) -> Option<String> {
    customer.and_then(|customer| customer.locale.clone())
}

#[cfg(not(feature = "mod-customer"))]
fn customer_preferred_locale(_customer: Option<&()>) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::{is_flag_enabled, parse_visibility};
    use loco_rs::task::Vars;
    use rustok_profiles::ProfileVisibility;

    #[test]
    fn parse_visibility_defaults_to_authenticated() {
        let vars = Vars::default();
        assert_eq!(
            parse_visibility(&vars).unwrap(),
            ProfileVisibility::Authenticated
        );
    }

    #[test]
    fn is_flag_enabled_accepts_true_like_values() {
        let mut vars = Vars::default();
        vars.cli.insert("dry_run".to_string(), "true".to_string());
        assert!(is_flag_enabled(&vars, "dry_run"));
    }
}
