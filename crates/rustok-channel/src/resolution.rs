use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ChannelDetailResponse, ChannelResult, ChannelService, ChannelTargetType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TargetSurface {
    #[default]
    Http,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelResolutionOrigin {
    HeaderId,
    HeaderSlug,
    Query,
    Host,
    Policy,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStage {
    HeaderId,
    HeaderSlug,
    Query,
    Host,
    Policy,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionOutcome {
    Matched,
    Miss,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionTraceStep {
    pub stage: ResolutionStage,
    pub outcome: ResolutionOutcome,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestFacts {
    pub tenant_id: Uuid,
    pub surface: TargetSurface,
    pub header_channel_id: Option<Uuid>,
    pub header_channel_slug: Option<String>,
    pub query_channel_slug: Option<String>,
    pub host: Option<String>,
    pub oauth_app_id: Option<Uuid>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionDecision {
    pub detail: Option<ChannelDetailResponse>,
    pub source: Option<ChannelResolutionOrigin>,
    pub trace: Vec<ResolutionTraceStep>,
}

impl ResolutionDecision {
    fn matched(
        detail: ChannelDetailResponse,
        source: ChannelResolutionOrigin,
        trace: Vec<ResolutionTraceStep>,
    ) -> Self {
        Self {
            detail: Some(detail),
            source: Some(source),
            trace,
        }
    }

    fn unresolved(trace: Vec<ResolutionTraceStep>) -> Self {
        Self {
            detail: None,
            source: None,
            trace,
        }
    }
}

pub struct ChannelResolver {
    service: ChannelService,
}

impl ChannelResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            service: ChannelService::new(db),
        }
    }

    pub async fn resolve(&self, facts: &RequestFacts) -> ChannelResult<ResolutionDecision> {
        let mut trace = Vec::new();

        if let Some(channel_id) = facts.header_channel_id {
            match self.service.get_channel_detail(channel_id).await? {
                detail if detail.channel.tenant_id != facts.tenant_id => {
                    trace.push(ResolutionTraceStep {
                        stage: ResolutionStage::HeaderId,
                        outcome: ResolutionOutcome::Rejected,
                        detail: format!(
                            "Channel '{channel_id}' does not belong to tenant '{}'",
                            facts.tenant_id
                        ),
                    });
                }
                detail if !detail.channel.is_active => {
                    trace.push(ResolutionTraceStep {
                        stage: ResolutionStage::HeaderId,
                        outcome: ResolutionOutcome::Rejected,
                        detail: format!("Channel '{channel_id}' is inactive"),
                    });
                }
                detail => {
                    trace.push(ResolutionTraceStep {
                        stage: ResolutionStage::HeaderId,
                        outcome: ResolutionOutcome::Matched,
                        detail: format!("Matched channel '{}'", detail.channel.slug),
                    });
                    return Ok(ResolutionDecision::matched(
                        detail,
                        ChannelResolutionOrigin::HeaderId,
                        trace,
                    ));
                }
            }
        } else {
            trace.push(ResolutionTraceStep {
                stage: ResolutionStage::HeaderId,
                outcome: ResolutionOutcome::Miss,
                detail: "No X-Channel-ID header on request".to_string(),
            });
        }

        if let Some(slug) = facts.header_channel_slug.as_deref() {
            match self
                .service
                .get_channel_detail_by_slug(facts.tenant_id, slug)
                .await?
            {
                Some(detail) if detail.channel.is_active => {
                    trace.push(ResolutionTraceStep {
                        stage: ResolutionStage::HeaderSlug,
                        outcome: ResolutionOutcome::Matched,
                        detail: format!("Matched channel '{slug}'"),
                    });
                    return Ok(ResolutionDecision::matched(
                        detail,
                        ChannelResolutionOrigin::HeaderSlug,
                        trace,
                    ));
                }
                Some(_) => trace.push(ResolutionTraceStep {
                    stage: ResolutionStage::HeaderSlug,
                    outcome: ResolutionOutcome::Rejected,
                    detail: format!("Channel '{slug}' is inactive"),
                }),
                None => trace.push(ResolutionTraceStep {
                    stage: ResolutionStage::HeaderSlug,
                    outcome: ResolutionOutcome::Miss,
                    detail: format!("No channel found for slug '{slug}'"),
                }),
            }
        } else {
            trace.push(ResolutionTraceStep {
                stage: ResolutionStage::HeaderSlug,
                outcome: ResolutionOutcome::Miss,
                detail: "No X-Channel-Slug header on request".to_string(),
            });
        }

        if let Some(slug) = facts.query_channel_slug.as_deref() {
            match self
                .service
                .get_channel_detail_by_slug(facts.tenant_id, slug)
                .await?
            {
                Some(detail) if detail.channel.is_active => {
                    trace.push(ResolutionTraceStep {
                        stage: ResolutionStage::Query,
                        outcome: ResolutionOutcome::Matched,
                        detail: format!("Matched channel '{slug}'"),
                    });
                    return Ok(ResolutionDecision::matched(
                        detail,
                        ChannelResolutionOrigin::Query,
                        trace,
                    ));
                }
                Some(_) => trace.push(ResolutionTraceStep {
                    stage: ResolutionStage::Query,
                    outcome: ResolutionOutcome::Rejected,
                    detail: format!("Channel '{slug}' is inactive"),
                }),
                None => trace.push(ResolutionTraceStep {
                    stage: ResolutionStage::Query,
                    outcome: ResolutionOutcome::Miss,
                    detail: format!("No channel found for slug '{slug}'"),
                }),
            }
        } else {
            trace.push(ResolutionTraceStep {
                stage: ResolutionStage::Query,
                outcome: ResolutionOutcome::Miss,
                detail: "No query channel selector on request".to_string(),
            });
        }

        trace.push(ResolutionTraceStep {
            stage: ResolutionStage::Policy,
            outcome: ResolutionOutcome::Miss,
            detail:
                "No tenant-scoped typed resolution policies are configured yet; built-in target resolution continues."
                    .to_string(),
        });

        if let Some(host) = facts.host.as_deref() {
            if let Some(normalized) = ChannelTargetType::WebDomain.normalize_value(host) {
                if let Some(detail) = self
                    .service
                    .get_channel_by_host_target_value(facts.tenant_id, normalized.as_str())
                    .await?
                {
                    trace.push(ResolutionTraceStep {
                        stage: ResolutionStage::Host,
                        outcome: ResolutionOutcome::Matched,
                        detail: format!("Matched host target '{normalized}'"),
                    });
                    return Ok(ResolutionDecision::matched(
                        detail,
                        ChannelResolutionOrigin::Host,
                        trace,
                    ));
                }

                trace.push(ResolutionTraceStep {
                    stage: ResolutionStage::Host,
                    outcome: ResolutionOutcome::Miss,
                    detail: format!("No host target matched '{normalized}'"),
                });
            } else {
                trace.push(ResolutionTraceStep {
                    stage: ResolutionStage::Host,
                    outcome: ResolutionOutcome::Rejected,
                    detail: format!("Host value '{host}' is not a valid canonical web_domain"),
                });
            }
        } else {
            trace.push(ResolutionTraceStep {
                stage: ResolutionStage::Host,
                outcome: ResolutionOutcome::Miss,
                detail: "No host value on request".to_string(),
            });
        }

        if let Some(detail) = self.service.get_default_channel(facts.tenant_id).await? {
            trace.push(ResolutionTraceStep {
                stage: ResolutionStage::Default,
                outcome: ResolutionOutcome::Matched,
                detail: format!("Using explicit tenant default '{}'", detail.channel.slug),
            });
            return Ok(ResolutionDecision::matched(
                detail,
                ChannelResolutionOrigin::Default,
                trace,
            ));
        }

        trace.push(ResolutionTraceStep {
            stage: ResolutionStage::Default,
            outcome: ResolutionOutcome::Miss,
            detail: "Tenant has no explicit default channel".to_string(),
        });

        Ok(ResolutionDecision::unresolved(trace))
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelResolutionOrigin, ChannelResolver, RequestFacts, ResolutionOutcome};
    use crate::{CreateChannelInput, CreateChannelTargetInput, migrations};
    use rustok_test_utils::setup_test_db;
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    async fn setup_channel_db() -> DatabaseConnection {
        let db = setup_test_db().await;
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE tenants (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                domain TEXT NULL UNIQUE,
                settings TEXT NOT NULL DEFAULT '{}',
                default_locale TEXT NOT NULL DEFAULT 'en',
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        ))
        .await
        .expect("tenants table should exist for channel foreign keys");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE o_auth_apps (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                app_type TEXT NOT NULL DEFAULT 'machine',
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        ))
        .await
        .expect("o_auth_apps table should exist for channel foreign keys");
        let manager = SchemaManager::new(&db);
        for migration in migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("channel migration should apply");
        }
        db
    }

    async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
        db.execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                tenant_id.into(),
                format!("{slug} tenant").into(),
                slug.to_string().into(),
                "{}".to_string().into(),
                "en".to_string().into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");
    }

    async fn create_channel(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) -> Uuid {
        crate::ChannelService::new(db.clone())
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: slug.to_string(),
                name: slug.to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created")
            .id
    }

    async fn add_web_target(db: &DatabaseConnection, channel_id: Uuid, host: &str) {
        crate::ChannelService::new(db.clone())
            .add_target(
                channel_id,
                CreateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: host.to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect("target should be created");
    }

    #[tokio::test]
    async fn resolver_prefers_explicit_selectors_before_host_and_default() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;

        let default_channel_id = create_channel(&db, tenant_id, "default").await;
        let header_channel_id = create_channel(&db, tenant_id, "header").await;
        let host_channel_id = create_channel(&db, tenant_id, "host").await;
        add_web_target(&db, host_channel_id, "shop.example.test").await;

        let resolver = ChannelResolver::new(db);
        let decision = resolver
            .resolve(&RequestFacts {
                tenant_id,
                header_channel_id: Some(header_channel_id),
                header_channel_slug: Some("missing".to_string()),
                query_channel_slug: Some("missing-query".to_string()),
                host: Some("shop.example.test".to_string()),
                ..RequestFacts::default()
            })
            .await
            .expect("resolution should succeed");

        assert_eq!(decision.source, Some(ChannelResolutionOrigin::HeaderId));
        assert_eq!(
            decision.detail.expect("detail").channel.id,
            header_channel_id
        );
        assert_ne!(header_channel_id, default_channel_id);
    }

    #[tokio::test]
    async fn resolver_returns_trace_for_unresolved_request() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;

        let resolver = ChannelResolver::new(db);
        let decision = resolver
            .resolve(&RequestFacts {
                tenant_id,
                host: Some("bad host".to_string()),
                ..RequestFacts::default()
            })
            .await
            .expect("resolution should succeed");

        assert!(decision.detail.is_none(), "request should stay unresolved");
        assert!(
            decision.source.is_none(),
            "unresolved request has no source"
        );
        assert!(
            decision
                .trace
                .iter()
                .any(|step| step.outcome == ResolutionOutcome::Rejected),
            "trace must explain rejected invalid host"
        );
    }
}
