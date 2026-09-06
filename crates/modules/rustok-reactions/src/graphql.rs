use std::sync::Arc;
use std::time::Duration;

use async_graphql::{Context, Enum, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, ChannelContext, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
    graphql::{GraphQLError, GraphqlRuntimeInputs, require_module_enabled},
};
use rustok_reactions_api::{
    ApplyReactionCommand, ReactionAction, ReactionCommandIdentity, ReactionKey, ReactionReadPort,
    ReactionReadRequest, ReactionSelectionPolicy, ReactionSnapshot, ReactionSourceSlug,
    ReactionSubjectKind, ReactionSubjectRef, ReactionSubjectRegistry, ReactionWritePort,
};
use uuid::Uuid;

use crate::ReactionsService;

const MODULE_SLUG: &str = "reactions";
const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct ReactionsGraphqlRuntimeData {
    service: ReactionsService,
}

pub fn attach_schema_data(
    inputs: &GraphqlRuntimeInputs,
) -> std::result::Result<ReactionsGraphqlRuntimeData, String> {
    let subjects = inputs
        .shared_get::<Arc<ReactionSubjectRegistry>>()
        .ok_or_else(|| {
            "Reactions subject registry is not registered in host runtime".to_string()
        })?;
    Ok(ReactionsGraphqlRuntimeData {
        service: ReactionsService::new(inputs.db_clone(), subjects),
    })
}

#[derive(Default)]
pub struct ReactionsQuery;

#[Object]
impl ReactionsQuery {
    /// Reads one exact revisioned reaction subject. Tenant and current actor are
    /// derived from request context rather than accepted from caller input.
    async fn reaction_snapshot(
        &self,
        ctx: &Context<'_>,
        subject: ReactionSubjectInput,
    ) -> Result<ReactionSnapshotGql> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let tenant = require_tenant(ctx)?;
        let auth = optional_principal(ctx, tenant)?;
        let subject = subject.into_subject(tenant.id)?;
        let actor_id = auth
            .filter(|auth| auth.is_human_user_principal())
            .map(|auth| auth.user_id);
        let request = ReactionReadRequest::new(subject, actor_id)
            .map_err(|_| bad_input("invalid reaction read request"))?;
        let service = runtime(ctx)?;
        let snapshot = ReactionReadPort::read_reactions(
            service,
            port_context(ctx, tenant, auth, None)?,
            request,
        )
        .await
        .map_err(map_port_error)?;
        Ok(ReactionSnapshotGql::from(snapshot))
    }
}

#[derive(Default)]
pub struct ReactionsMutation;

#[Object]
impl ReactionsMutation {
    /// Applies one bounded reaction transition. The authenticated user is the
    /// actor and commandId is also the owner idempotency key.
    async fn apply_reaction(
        &self,
        ctx: &Context<'_>,
        input: ApplyReactionInput,
    ) -> Result<ReactionWriteResultGql> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let tenant = require_tenant(ctx)?;
        let auth = require_human_user(ctx, tenant)?;
        let command_id = input.command_id;
        let subject = input.subject.into_subject(tenant.id)?;
        let identity = ReactionCommandIdentity::new(command_id, auth.user_id)
            .map_err(|_| bad_input("invalid reaction command identity"))?;
        let reaction =
            ReactionKey::new(input.reaction).map_err(|_| bad_input("invalid reaction key"))?;
        let command = ApplyReactionCommand::new(identity, subject, reaction, input.action.into());
        let service = runtime(ctx)?;
        let receipt = ReactionWritePort::apply_reaction(
            service,
            port_context(ctx, tenant, Some(auth), Some(command_id.to_string()))?,
            command,
        )
        .await
        .map_err(map_port_error)?;

        Ok(ReactionWriteResultGql {
            command_id: receipt.command_id(),
            changed: receipt.changed(),
        })
    }
}

#[derive(Clone, Debug, InputObject)]
pub struct ReactionSubjectInput {
    pub source: String,
    pub kind: String,
    pub subject_id: Uuid,
    /// Positive u64 encoded as a decimal string so GraphQL's signed 32-bit Int
    /// scalar cannot truncate owner revisions.
    pub subject_revision: String,
}

impl ReactionSubjectInput {
    fn into_subject(self, tenant_id: Uuid) -> Result<ReactionSubjectRef> {
        let revision = parse_positive_u64(&self.subject_revision, "subjectRevision")?;
        let source = ReactionSourceSlug::new(self.source)
            .map_err(|_| bad_input("invalid reaction source"))?;
        let kind = ReactionSubjectKind::new(self.kind)
            .map_err(|_| bad_input("invalid reaction subject kind"))?;
        ReactionSubjectRef::new(tenant_id, source, kind, self.subject_id, revision)
            .map_err(|_| bad_input("invalid reaction subject"))
    }
}

#[derive(Clone, Debug, InputObject)]
pub struct ApplyReactionInput {
    pub command_id: Uuid,
    pub subject: ReactionSubjectInput,
    pub reaction: String,
    pub action: ReactionActionGql,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum ReactionActionGql {
    Add,
    Remove,
}

impl From<ReactionActionGql> for ReactionAction {
    fn from(value: ReactionActionGql) -> Self {
        match value {
            ReactionActionGql::Add => Self::Add,
            ReactionActionGql::Remove => Self::Remove,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum ReactionSelectionModeGql {
    Single,
    Multiple,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct ReactionSubjectGql {
    pub source: String,
    pub kind: String,
    pub subject_id: Uuid,
    pub subject_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct ReactionCatalogGql {
    pub selection_mode: ReactionSelectionModeGql,
    pub max_selected: i32,
    pub keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct ReactionActorStateGql {
    pub revision: String,
    pub selected: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct ReactionAggregateGql {
    pub reaction: String,
    pub count: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct ReactionSnapshotGql {
    pub subject: ReactionSubjectGql,
    pub catalog: ReactionCatalogGql,
    pub actor_state: Option<ReactionActorStateGql>,
    pub aggregates: Vec<ReactionAggregateGql>,
}

impl From<ReactionSnapshot> for ReactionSnapshotGql {
    fn from(snapshot: ReactionSnapshot) -> Self {
        let subject = snapshot.subject();
        let catalog = snapshot.catalog();
        let selection = catalog.selection();
        let (selection_mode, max_selected) = match selection {
            ReactionSelectionPolicy::Single => (ReactionSelectionModeGql::Single, 1),
            ReactionSelectionPolicy::Multiple { max_selected } => {
                (ReactionSelectionModeGql::Multiple, i32::from(max_selected))
            }
        };
        let actor_state = snapshot.actor_state().map(|state| ReactionActorStateGql {
            revision: state.revision().to_string(),
            selected: state
                .selected()
                .iter()
                .map(|key| key.as_str().to_string())
                .collect(),
        });
        let aggregates = snapshot
            .aggregates()
            .iter()
            .map(|aggregate| ReactionAggregateGql {
                reaction: aggregate.reaction.as_str().to_string(),
                count: aggregate.count.to_string(),
            })
            .collect();

        Self {
            subject: ReactionSubjectGql {
                source: subject.source().as_str().to_string(),
                kind: subject.kind().as_str().to_string(),
                subject_id: subject.subject_id(),
                subject_revision: subject.subject_revision().to_string(),
            },
            catalog: ReactionCatalogGql {
                selection_mode,
                max_selected,
                keys: catalog
                    .keys()
                    .iter()
                    .map(|key| key.as_str().to_string())
                    .collect(),
            },
            actor_state,
            aggregates,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct ReactionWriteResultGql {
    pub command_id: Uuid,
    pub changed: bool,
}

fn runtime<'a>(ctx: &'a Context<'a>) -> Result<&'a ReactionsService> {
    ctx.data::<ReactionsGraphqlRuntimeData>()
        .map(|runtime| &runtime.service)
        .map_err(|_| {
            <FieldError as GraphQLError>::internal_error(
                "Reactions GraphQL runtime data is not registered",
            )
        })
}

fn require_tenant<'a>(ctx: &'a Context<'a>) -> Result<&'a TenantContext> {
    ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Reactions tenant context is not registered")
    })
}

fn optional_principal<'a>(
    ctx: &'a Context<'a>,
    tenant: &TenantContext,
) -> Result<Option<&'a AuthContext>> {
    let Some(auth) = ctx.data_opt::<AuthContext>() else {
        return Ok(None);
    };
    ensure_principal_tenant(auth, tenant)?;
    Ok(Some(auth))
}

fn require_human_user<'a>(ctx: &'a Context<'a>, tenant: &TenantContext) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    ensure_principal_tenant(auth, tenant)?;
    if auth.is_service_principal() {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "reaction writes require human-user credentials",
        ));
    }
    Ok(auth)
}

fn ensure_principal_tenant(auth: &AuthContext, tenant: &TenantContext) -> Result<()> {
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "reaction operations must use the authenticated tenant",
        ));
    }
    Ok(())
}

fn port_context(
    ctx: &Context<'_>,
    tenant: &TenantContext,
    auth: Option<&AuthContext>,
    idempotency_key: Option<String>,
) -> Result<PortContext> {
    let locale = ctx
        .data_opt::<RequestContext>()
        .map(|request| request.locale.clone())
        .unwrap_or_else(|| tenant.default_locale.clone());
    let actor = auth
        .map(AuthContext::port_actor)
        .unwrap_or_else(PortActor::system);
    let mut context = PortContext::new(
        tenant.id.to_string(),
        actor,
        locale,
        format!("graphql-reactions-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);

    if let Some(auth) = auth {
        for permission in &auth.permissions {
            context = context.with_claim(permission.to_string());
        }
    }
    if let Some(channel) = ctx.data_opt::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
    } else if let Some(channel) = ctx
        .data_opt::<RequestContext>()
        .and_then(|request| request.channel_slug.clone())
    {
        context = context.with_channel(channel);
    }
    if let Some(key) = idempotency_key {
        context = context.with_idempotency_key(key);
    }
    Ok(context)
}

fn parse_positive_u64(value: &str, field: &str) -> Result<u64> {
    let revision = value.trim().parse::<u64>().map_err(|_| {
        bad_input(&format!(
            "{field} must be a positive unsigned 64-bit integer string"
        ))
    })?;
    if revision == 0 {
        return Err(bad_input(&format!(
            "{field} must be a positive unsigned 64-bit integer string"
        )));
    }
    Ok(revision)
}

fn bad_input(message: &str) -> FieldError {
    <FieldError as GraphQLError>::bad_user_input(message)
}

fn map_port_error(error: PortError) -> FieldError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Conflict => {
            <FieldError as GraphQLError>::bad_user_input(&error.message)
        }
        PortErrorKind::NotFound => <FieldError as GraphQLError>::not_found(&error.message),
        PortErrorKind::Forbidden => <FieldError as GraphQLError>::permission_denied(&error.message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Reactions service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => {
            <FieldError as GraphQLError>::internal_error("Reactions operation requires review")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_positive_u64;

    #[test]
    fn subject_revision_uses_positive_u64_strings() {
        assert_eq!(parse_positive_u64("1", "subjectRevision").unwrap(), 1);
        assert_eq!(
            parse_positive_u64("18446744073709551615", "subjectRevision").unwrap(),
            u64::MAX
        );
        assert!(parse_positive_u64("0", "subjectRevision").is_err());
        assert!(parse_positive_u64("-1", "subjectRevision").is_err());
        assert!(parse_positive_u64("not-a-revision", "subjectRevision").is_err());
    }
}
