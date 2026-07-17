use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;
use validator::Validate;

use crate::command_receipts::{
    admit_command, command_request_hash, complete_command, normalize_idempotency_key,
    replay_command, rollback_command, CommandReceiptAdmission,
};
use crate::dto::{
    AddMarketplaceSellerMemberInput, CreateMarketplaceSellerInput,
    MarketplaceSellerMemberResponse, MarketplaceSellerMemberRole, MarketplaceSellerMemberStatus,
    MarketplaceSellerOnboardingStatus, MarketplaceSellerResponse, MarketplaceSellerStatus,
    ReviewMarketplaceSellerOnboardingInput, SubmitMarketplaceSellerOnboardingInput,
    SuspendMarketplaceSellerInput, UpdateMarketplaceSellerMemberInput,
    UpdateMarketplaceSellerProfileInput,
};
use crate::entities::{seller, seller_member};
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};
use crate::MarketplaceSellerService;

const RESPONSE_KIND_SELLER: &str = "seller";
const RESPONSE_KIND_MEMBER: &str = "member";

impl MarketplaceSellerService {
    pub(crate) async fn create_seller_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: CreateMarketplaceSellerInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let handle = normalize_handle(input.handle.as_str())?;
        let display_name = required_text(input.display_name, "display_name")?;
        let legal_name = optional_text(input.legal_name);
        let metadata = object_or_empty(input.metadata, "metadata")?;
        let normalized = serde_json::json!({
            "handle": handle,
            "display_name": display_name,
            "legal_name": legal_name,
            "owner_user_id": input.owner_user_id,
            "metadata": metadata,
        });
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash("create_seller", actor_id, &normalized)?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            "create_seller",
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                "create_seller",
                hash.as_str(),
                RESPONSE_KIND_SELLER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result: MarketplaceSellerResult<MarketplaceSellerResponse> = async {
                    if seller::Entity::find()
                        .filter(seller::Column::TenantId.eq(tenant_id))
                        .filter(seller::Column::Handle.eq(handle.as_str()))
                        .one(&receipt.transaction)
                        .await?
                        .is_some()
                    {
                        return Err(MarketplaceSellerError::DuplicateHandle(handle.clone()));
                    }
                    let seller_id = generate_id();
                    let member_id = generate_id();
                    let now = Utc::now();
                    let seller_model = seller::ActiveModel {
                        id: Set(seller_id),
                        tenant_id: Set(tenant_id),
                        handle: Set(handle.clone()),
                        display_name: Set(display_name.clone()),
                        legal_name: Set(legal_name.clone()),
                        status: Set(MarketplaceSellerStatus::Draft.as_str().to_string()),
                        onboarding_status: Set(
                            MarketplaceSellerOnboardingStatus::Draft
                                .as_str()
                                .to_string(),
                        ),
                        onboarding_note: Set(None),
                        suspension_reason: Set(None),
                        metadata: Set(metadata.clone()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                        activated_at: Set(None),
                        suspended_at: Set(None),
                    }
                    .insert(&receipt.transaction)
                    .await
                    .map_err(|error| {
                        if is_unique_constraint(&error) {
                            MarketplaceSellerError::DuplicateHandle(handle.clone())
                        } else {
                            error.into()
                        }
                    })?;
                    seller_member::ActiveModel {
                        id: Set(member_id),
                        tenant_id: Set(tenant_id),
                        seller_id: Set(seller_id),
                        user_id: Set(input.owner_user_id),
                        role: Set(MarketplaceSellerMemberRole::Owner.as_str().to_string()),
                        status: Set(MarketplaceSellerMemberStatus::Active.as_str().to_string()),
                        invited_by_actor_id: Set(Some(actor_id)),
                        accepted_at: Set(Some(now.into())),
                        metadata: Set(serde_json::json!({"source": "seller_creation"})),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    }
                    .insert(&receipt.transaction)
                    .await?;
                    map_seller(seller_model)
                }
                .await;
                match result {
                    Ok(response) => {
                        complete_command(receipt, RESPONSE_KIND_SELLER, &response).await
                    }
                    Err(error) => rollback_command(receipt, error).await,
                }
            }
        }
    }

    pub(crate) async fn update_profile_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        seller_id: Uuid,
        input: UpdateMarketplaceSellerProfileInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let display_name = input
            .display_name
            .map(|value| required_text(value, "display_name"))
            .transpose()?;
        let legal_name_present = input.legal_name.is_some();
        let legal_name = optional_text(input.legal_name);
        let metadata = input
            .metadata
            .map(|value| object_or_empty(value, "metadata"))
            .transpose()?;
        let normalized = serde_json::json!({
            "seller_id": seller_id,
            "display_name": display_name,
            "legal_name_present": legal_name_present,
            "legal_name": legal_name,
            "metadata": metadata,
        });
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash("update_seller_profile", actor_id, &normalized)?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            "update_seller_profile",
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                "update_seller_profile",
                hash.as_str(),
                RESPONSE_KIND_SELLER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result: MarketplaceSellerResult<MarketplaceSellerResponse> = async {
                    let current = find_seller(&receipt.transaction, tenant_id, seller_id).await?;
                    if current.status == MarketplaceSellerStatus::Closed.as_str() {
                        return Err(MarketplaceSellerError::InvalidTransition {
                            from: current.status,
                            to: "profile_updated".to_string(),
                        });
                    }
                    let mut active: seller::ActiveModel = current.into();
                    if let Some(display_name) = display_name.clone() {
                        active.display_name = Set(display_name);
                    }
                    if legal_name_present {
                        active.legal_name = Set(legal_name.clone());
                    }
                    if let Some(metadata) = metadata.clone() {
                        active.metadata = Set(metadata);
                    }
                    active.updated_at = Set(Utc::now().into());
                    map_seller(active.update(&receipt.transaction).await?)
                }
                .await;
                match result {
                    Ok(response) => {
                        complete_command(receipt, RESPONSE_KIND_SELLER, &response).await
                    }
                    Err(error) => rollback_command(receipt, error).await,
                }
            }
        }
    }

    pub(crate) async fn submit_onboarding_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        seller_id: Uuid,
        input: SubmitMarketplaceSellerOnboardingInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let note = optional_text(input.note);
        self.execute_seller_transition_receipt(
            tenant_id,
            actor_id,
            idempotency_key,
            "submit_seller_onboarding",
            serde_json::json!({"seller_id": seller_id, "note": note}),
            seller_id,
            |transaction| {
                Box::pin(async move {
                    let result = seller::Entity::update_many()
                        .col_expr(
                            seller::Column::OnboardingStatus,
                            sea_orm::sea_query::Expr::value(
                                MarketplaceSellerOnboardingStatus::Submitted.as_str(),
                            ),
                        )
                        .col_expr(
                            seller::Column::OnboardingNote,
                            sea_orm::sea_query::Expr::value(note),
                        )
                        .col_expr(
                            seller::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::current_timestamp().into(),
                        )
                        .filter(seller::Column::TenantId.eq(tenant_id))
                        .filter(seller::Column::Id.eq(seller_id))
                        .filter(
                            seller::Column::Status.eq(MarketplaceSellerStatus::Draft.as_str()),
                        )
                        .filter(
                            seller::Column::OnboardingStatus.is_in([
                                MarketplaceSellerOnboardingStatus::Draft.as_str(),
                                MarketplaceSellerOnboardingStatus::Rejected.as_str(),
                            ]),
                        )
                        .exec(transaction)
                        .await?;
                    require_transition(
                        transaction,
                        result.rows_affected,
                        tenant_id,
                        seller_id,
                        "submitted",
                    )
                    .await
                })
            },
        )
        .await
    }

    pub(crate) async fn review_onboarding_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        seller_id: Uuid,
        input: ReviewMarketplaceSellerOnboardingInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let approved = input.approved;
        let note = optional_text(input.note);
        self.execute_seller_transition_receipt(
            tenant_id,
            actor_id,
            idempotency_key,
            "review_seller_onboarding",
            serde_json::json!({"seller_id": seller_id, "approved": approved, "note": note}),
            seller_id,
            move |transaction| {
                Box::pin(async move {
                    let onboarding = if approved {
                        MarketplaceSellerOnboardingStatus::Approved
                    } else {
                        MarketplaceSellerOnboardingStatus::Rejected
                    };
                    let next_status = if approved {
                        MarketplaceSellerStatus::Active
                    } else {
                        MarketplaceSellerStatus::Draft
                    };
                    let now = Utc::now().fixed_offset();
                    let mut update = seller::Entity::update_many()
                        .col_expr(
                            seller::Column::OnboardingStatus,
                            sea_orm::sea_query::Expr::value(onboarding.as_str()),
                        )
                        .col_expr(
                            seller::Column::Status,
                            sea_orm::sea_query::Expr::value(next_status.as_str()),
                        )
                        .col_expr(
                            seller::Column::OnboardingNote,
                            sea_orm::sea_query::Expr::value(note),
                        )
                        .col_expr(
                            seller::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::value(now),
                        )
                        .filter(seller::Column::TenantId.eq(tenant_id))
                        .filter(seller::Column::Id.eq(seller_id))
                        .filter(
                            seller::Column::Status.eq(MarketplaceSellerStatus::Draft.as_str()),
                        )
                        .filter(
                            seller::Column::OnboardingStatus.eq(
                                MarketplaceSellerOnboardingStatus::Submitted.as_str(),
                            ),
                        );
                    if approved {
                        update = update.col_expr(
                            seller::Column::ActivatedAt,
                            sea_orm::sea_query::Expr::value(Some(now)),
                        );
                    }
                    let result = update.exec(transaction).await?;
                    require_transition(
                        transaction,
                        result.rows_affected,
                        tenant_id,
                        seller_id,
                        onboarding.as_str(),
                    )
                    .await
                })
            },
        )
        .await
    }

    pub(crate) async fn suspend_seller_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        seller_id: Uuid,
        input: SuspendMarketplaceSellerInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let reason = required_text(input.reason, "reason")?;
        self.execute_seller_transition_receipt(
            tenant_id,
            actor_id,
            idempotency_key,
            "suspend_seller",
            serde_json::json!({"seller_id": seller_id, "reason": reason}),
            seller_id,
            move |transaction| {
                Box::pin(async move {
                    let now = Utc::now().fixed_offset();
                    let result = seller::Entity::update_many()
                        .col_expr(
                            seller::Column::Status,
                            sea_orm::sea_query::Expr::value(
                                MarketplaceSellerStatus::Suspended.as_str(),
                            ),
                        )
                        .col_expr(
                            seller::Column::SuspensionReason,
                            sea_orm::sea_query::Expr::value(Some(reason)),
                        )
                        .col_expr(
                            seller::Column::SuspendedAt,
                            sea_orm::sea_query::Expr::value(Some(now)),
                        )
                        .col_expr(
                            seller::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::value(now),
                        )
                        .filter(seller::Column::TenantId.eq(tenant_id))
                        .filter(seller::Column::Id.eq(seller_id))
                        .filter(
                            seller::Column::Status.eq(MarketplaceSellerStatus::Active.as_str()),
                        )
                        .exec(transaction)
                        .await?;
                    require_transition(
                        transaction,
                        result.rows_affected,
                        tenant_id,
                        seller_id,
                        "suspended",
                    )
                    .await
                })
            },
        )
        .await
    }

    pub(crate) async fn reactivate_seller_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        seller_id: Uuid,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        self.execute_seller_transition_receipt(
            tenant_id,
            actor_id,
            idempotency_key,
            "reactivate_seller",
            serde_json::json!({"seller_id": seller_id}),
            seller_id,
            |transaction| {
                Box::pin(async move {
                    let result = seller::Entity::update_many()
                        .col_expr(
                            seller::Column::Status,
                            sea_orm::sea_query::Expr::value(
                                MarketplaceSellerStatus::Active.as_str(),
                            ),
                        )
                        .col_expr(
                            seller::Column::SuspensionReason,
                            sea_orm::sea_query::Expr::value(Option::<String>::None),
                        )
                        .col_expr(
                            seller::Column::SuspendedAt,
                            sea_orm::sea_query::Expr::value(
                                Option::<chrono::DateTime<chrono::FixedOffset>>::None,
                            ),
                        )
                        .col_expr(
                            seller::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::current_timestamp().into(),
                        )
                        .filter(seller::Column::TenantId.eq(tenant_id))
                        .filter(seller::Column::Id.eq(seller_id))
                        .filter(
                            seller::Column::Status.eq(
                                MarketplaceSellerStatus::Suspended.as_str(),
                            ),
                        )
                        .filter(
                            seller::Column::OnboardingStatus.eq(
                                MarketplaceSellerOnboardingStatus::Approved.as_str(),
                            ),
                        )
                        .exec(transaction)
                        .await?;
                    require_transition(
                        transaction,
                        result.rows_affected,
                        tenant_id,
                        seller_id,
                        "active",
                    )
                    .await
                })
            },
        )
        .await
    }

    pub(crate) async fn add_member_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        seller_id: Uuid,
        input: AddMarketplaceSellerMemberInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerMemberResponse> {
        let metadata = object_or_empty(input.metadata, "metadata")?;
        let normalized = serde_json::json!({
            "seller_id": seller_id,
            "user_id": input.user_id,
            "role": input.role,
            "metadata": metadata,
        });
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash("add_seller_member", actor_id, &normalized)?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            "add_seller_member",
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                "add_seller_member",
                hash.as_str(),
                RESPONSE_KIND_MEMBER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result: MarketplaceSellerResult<MarketplaceSellerMemberResponse> = async {
                    find_seller(&receipt.transaction, tenant_id, seller_id).await?;
                    if seller_member::Entity::find()
                        .filter(seller_member::Column::TenantId.eq(tenant_id))
                        .filter(seller_member::Column::SellerId.eq(seller_id))
                        .filter(seller_member::Column::UserId.eq(input.user_id))
                        .one(&receipt.transaction)
                        .await?
                        .is_some()
                    {
                        return Err(MarketplaceSellerError::DuplicateMembership {
                            seller_id,
                            user_id: input.user_id,
                        });
                    }
                    let now = Utc::now();
                    let model = seller_member::ActiveModel {
                        id: Set(generate_id()),
                        tenant_id: Set(tenant_id),
                        seller_id: Set(seller_id),
                        user_id: Set(input.user_id),
                        role: Set(input.role.as_str().to_string()),
                        status: Set(
                            MarketplaceSellerMemberStatus::Invited
                                .as_str()
                                .to_string(),
                        ),
                        invited_by_actor_id: Set(Some(actor_id)),
                        accepted_at: Set(None),
                        metadata: Set(metadata.clone()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    }
                    .insert(&receipt.transaction)
                    .await
                    .map_err(|error| {
                        if is_unique_constraint(&error) {
                            MarketplaceSellerError::DuplicateMembership {
                                seller_id,
                                user_id: input.user_id,
                            }
                        } else {
                            error.into()
                        }
                    })?;
                    map_member(model)
                }
                .await;
                match result {
                    Ok(response) => {
                        complete_command(receipt, RESPONSE_KIND_MEMBER, &response).await
                    }
                    Err(error) => rollback_command(receipt, error).await,
                }
            }
        }
    }

    pub(crate) async fn update_member_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        seller_id: Uuid,
        member_id: Uuid,
        input: UpdateMarketplaceSellerMemberInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerMemberResponse> {
        let metadata = input
            .metadata
            .map(|value| object_or_empty(value, "metadata"))
            .transpose()?;
        let normalized = serde_json::json!({
            "seller_id": seller_id,
            "member_id": member_id,
            "role": input.role,
            "status": input.status,
            "metadata": metadata,
        });
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash("update_seller_member", actor_id, &normalized)?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            "update_seller_member",
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                "update_seller_member",
                hash.as_str(),
                RESPONSE_KIND_MEMBER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result: MarketplaceSellerResult<MarketplaceSellerMemberResponse> = async {
                    let current = seller_member::Entity::find_by_id(member_id)
                        .filter(seller_member::Column::TenantId.eq(tenant_id))
                        .filter(seller_member::Column::SellerId.eq(seller_id))
                        .one(&receipt.transaction)
                        .await?
                        .ok_or(MarketplaceSellerError::MemberNotFound(member_id))?;
                    if current.role == MarketplaceSellerMemberRole::Owner.as_str()
                        && matches!(
                            input.role,
                            Some(role) if role != MarketplaceSellerMemberRole::Owner
                        )
                    {
                        return Err(MarketplaceSellerError::Validation(
                            "owner membership role cannot be changed".to_string(),
                        ));
                    }
                    if current.role == MarketplaceSellerMemberRole::Owner.as_str()
                        && input.status == Some(MarketplaceSellerMemberStatus::Disabled)
                    {
                        return Err(MarketplaceSellerError::Validation(
                            "owner membership cannot be disabled".to_string(),
                        ));
                    }
                    let mut active: seller_member::ActiveModel = current.into();
                    if let Some(role) = input.role {
                        active.role = Set(role.as_str().to_string());
                    }
                    if let Some(status) = input.status {
                        active.status = Set(status.as_str().to_string());
                        if status == MarketplaceSellerMemberStatus::Active {
                            active.accepted_at = Set(Some(Utc::now().into()));
                        }
                    }
                    if let Some(metadata) = metadata.clone() {
                        active.metadata = Set(metadata);
                    }
                    active.updated_at = Set(Utc::now().into());
                    map_member(active.update(&receipt.transaction).await?)
                }
                .await;
                match result {
                    Ok(response) => {
                        complete_command(receipt, RESPONSE_KIND_MEMBER, &response).await
                    }
                    Err(error) => rollback_command(receipt, error).await,
                }
            }
        }
    }

    async fn execute_seller_transition_receipt<F>(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        command_kind: &'static str,
        normalized_request: serde_json::Value,
        _seller_id: Uuid,
        execute: F,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse>
    where
        F: for<'a> FnOnce(
            &'a sea_orm::DatabaseTransaction,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = MarketplaceSellerResult<MarketplaceSellerResponse>,
                    > + Send
                    + 'a,
            >,
        >,
    {
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash(command_kind, actor_id, &normalized_request)?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            command_kind,
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                command_kind,
                hash.as_str(),
                RESPONSE_KIND_SELLER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result = execute(&receipt.transaction).await;
                match result {
                    Ok(response) => {
                        complete_command(receipt, RESPONSE_KIND_SELLER, &response).await
                    }
                    Err(error) => rollback_command(receipt, error).await,
                }
            }
        }
    }
}

async fn find_seller(
    connection: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    seller_id: Uuid,
) -> MarketplaceSellerResult<seller::Model> {
    seller::Entity::find_by_id(seller_id)
        .filter(seller::Column::TenantId.eq(tenant_id))
        .one(connection)
        .await?
        .ok_or(MarketplaceSellerError::SellerNotFound(seller_id))
}

async fn require_transition(
    connection: &sea_orm::DatabaseTransaction,
    rows_affected: u64,
    tenant_id: Uuid,
    seller_id: Uuid,
    to: &str,
) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
    let current = find_seller(connection, tenant_id, seller_id).await?;
    if rows_affected == 1 {
        return map_seller(current);
    }
    Err(MarketplaceSellerError::InvalidTransition {
        from: format!("{}:{}", current.status, current.onboarding_status),
        to: to.to_string(),
    })
}

fn map_seller(model: seller::Model) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
    let status = MarketplaceSellerStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceSellerError::Validation(format!(
            "unknown marketplace seller status `{}`",
            model.status
        ))
    })?;
    let onboarding_status = MarketplaceSellerOnboardingStatus::parse(model.onboarding_status.as_str())
        .ok_or_else(|| {
            MarketplaceSellerError::Validation(format!(
                "unknown marketplace seller onboarding status `{}`",
                model.onboarding_status
            ))
        })?;
    Ok(MarketplaceSellerResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        handle: model.handle,
        display_name: model.display_name,
        legal_name: model.legal_name,
        status,
        onboarding_status,
        onboarding_note: model.onboarding_note,
        suspension_reason: model.suspension_reason,
        metadata: model.metadata,
        created_at: model.created_at,
        updated_at: model.updated_at,
        activated_at: model.activated_at,
        suspended_at: model.suspended_at,
    })
}

fn map_member(
    model: seller_member::Model,
) -> MarketplaceSellerResult<MarketplaceSellerMemberResponse> {
    let role = MarketplaceSellerMemberRole::parse(model.role.as_str()).ok_or_else(|| {
        MarketplaceSellerError::Validation(format!(
            "unknown marketplace seller member role `{}`",
            model.role
        ))
    })?;
    let status = MarketplaceSellerMemberStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceSellerError::Validation(format!(
            "unknown marketplace seller member status `{}`",
            model.status
        ))
    })?;
    Ok(MarketplaceSellerMemberResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        seller_id: model.seller_id,
        user_id: model.user_id,
        role,
        status,
        invited_by_actor_id: model.invited_by_actor_id,
        accepted_at: model.accepted_at,
        metadata: model.metadata,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

fn normalize_handle(value: &str) -> MarketplaceSellerResult<String> {
    let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
    if normalized.len() < 2
        || normalized.len() > 80
        || normalized.starts_with('-')
        || normalized.ends_with('-')
        || normalized.chars().any(|character| {
            !(character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-')
        })
    {
        return Err(MarketplaceSellerError::Validation(
            "handle must contain 2 to 80 lowercase ASCII letters, digits, or internal hyphens"
                .to_string(),
        ));
    }
    Ok(normalized)
}

fn required_text(value: String, field: &str) -> MarketplaceSellerResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(MarketplaceSellerError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn object_or_empty(
    value: serde_json::Value,
    field: &str,
) -> MarketplaceSellerResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplaceSellerError::Validation(format!(
            "{field} must be a JSON object"
        ))),
    }
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
