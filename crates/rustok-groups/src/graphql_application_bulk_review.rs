const BULK_REVIEW_PORT_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct GroupsApplicationBulkReviewMutation;

#[async_graphql::Object]
impl GroupsApplicationBulkReviewMutation {
    async fn bulk_review_group_membership_applications(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: BulkReviewGroupMembershipApplicationsInputGql,
    ) -> Result<BulkReviewGroupMembershipApplicationsResultGql> {
        let auth = require_authenticated(ctx)?;
        let result = crate::GroupApplicationBulkReviewCommandPort::bulk_review_group_membership_applications(
            &application_service(ctx)?,
            bulk_review_port_context(ctx, auth, idempotency_key)?,
            crate::BulkReviewGroupMembershipApplicationsRequest {
                application_ids: input.application_ids,
                decision: input.decision.into(),
                note: input.note,
                confirmed: input.confirmed,
            },
        )
        .await
        .map_err(map_port_error)?;
        Ok(result.into())
    }
}

#[derive(async_graphql::InputObject)]
pub struct BulkReviewGroupMembershipApplicationsInputGql {
    pub application_ids: Vec<Uuid>,
    pub decision: GroupApplicationReviewDecisionGql,
    pub note: Option<String>,
    pub confirmed: bool,
}

#[derive(async_graphql::SimpleObject)]
pub struct BulkReviewGroupMembershipApplicationErrorGql {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(async_graphql::SimpleObject)]
pub struct BulkReviewGroupMembershipApplicationItemResultGql {
    pub application_id: Uuid,
    pub result: Option<ReviewGroupMembershipApplicationResultGql>,
    pub error: Option<BulkReviewGroupMembershipApplicationErrorGql>,
}

#[derive(async_graphql::SimpleObject)]
pub struct BulkReviewGroupMembershipApplicationsResultGql {
    pub items: Vec<BulkReviewGroupMembershipApplicationItemResultGql>,
    pub succeeded: u32,
    pub failed: u32,
}

impl From<crate::BulkReviewGroupMembershipApplicationsResult>
    for BulkReviewGroupMembershipApplicationsResultGql
{
    fn from(value: crate::BulkReviewGroupMembershipApplicationsResult) -> Self {
        Self {
            items: value
                .items
                .into_iter()
                .map(|item| BulkReviewGroupMembershipApplicationItemResultGql {
                    application_id: item.application_id,
                    result: item.result.map(Into::into),
                    error: item
                        .error
                        .map(|error| BulkReviewGroupMembershipApplicationErrorGql {
                            code: error.code,
                            message: error.message,
                            retryable: error.retryable,
                        }),
                })
                .collect(),
            succeeded: value.succeeded,
            failed: value.failed,
        }
    }
}

fn bulk_review_port_context(
    ctx: &Context<'_>,
    auth: &AuthContext,
    idempotency_key: String,
) -> Result<PortContext> {
    if idempotency_key.trim().is_empty() {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "groups application bulk-review idempotency key is required",
        ));
    }
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
    let locale = ctx
        .data::<RequestContext>()
        .map(|request| request.locale.clone())
        .or_else(|_| {
            ctx.data::<rustok_core::Locale>()
                .map(|locale| locale.as_str().to_string())
        })
        .unwrap_or_else(|_| tenant.default_locale.clone());
    let mut context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("graphql-groups-application-bulk-review-{}", Uuid::new_v4()),
    )
    .with_deadline(BULK_REVIEW_PORT_DEADLINE)
    .with_idempotency_key(idempotency_key);
    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Ok(channel) = ctx.data::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
    }
    Ok(context)
}
