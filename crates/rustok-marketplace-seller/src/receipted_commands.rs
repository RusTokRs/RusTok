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
    AddMarketplaceSellerMemberInput, CreateMarketplaceSellerInput, MarketplaceSellerMemberResponse,
    MarketplaceSellerMemberRole, MarketplaceSellerMemberStatus, MarketplaceSellerOnboardingStatus,
    MarketplaceSellerResponse, MarketplaceSellerStatus, ReviewMarketplaceSellerOnboardingInput,
    SubmitMarketplaceSellerOnboardingInput, SuspendMarketplaceSellerInput,
    UpdateMarketplaceSellerMemberInput, UpdateMarketplaceSellerProfileInput,
};
use crate::entities::{seller, seller_member};
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};
use crate::seller_events::append_receipted_member_event;
use crate::service::{
    find_seller, is_unique_constraint, load_seller_response, map_member, normalize_handle,
    normalize_seller_locale, object_or_empty, optional_text, required_text, upsert_translation,
    validate_owner_membership_update,
};
use crate::MarketplaceSellerService;

const RESPONSE_KIND_SELLER: &str = "seller";
const RESPONSE_KIND_MEMBER: &str = "member";

impl MarketplaceSellerService {
    pub(crate) async fn create_seller_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        locale: &str,
        input: CreateMarketplaceSellerInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let locale = normalize_seller_locale(locale)?;
        let handle = normalize_handle(input.handle.as_str())?;
        let display_name = required_text(input.display_name, "display_name")?;
        let legal_name = optional_text(input.legal_name);
        let metadata = object_or_empty(input.metadata, "metadata")?;
        let normalized = serde_json::json!({
            "locale": locale,
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
                    let now = Utc::now();
                    let seller_id = generate_id();
                    seller::ActiveModel {
                        id: Set(seller_id),
                        tenant_id: Set(tenant_id),
                        handle: Set(handle.clone()),
                        legal_name: Set(legal_name.clone()),
                        status: Set(MarketplaceSellerStatus::Draft.as_str().to_string()),
                        onboarding_status: Set(MarketplaceSellerOnboardingStatus::Draft
                            .as_str()
                            .to_string()),
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
                    upsert_translation(
                        &receipt.transaction,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                        display_name.clone(),
                    )
                    .await?;
                    seller_member::ActiveModel {
                        id: Set(generate_id()),
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
                    load_seller_response(
                        &receipt.transaction,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                    )
                    .await
                }
                .await;
                finish_seller_command(receipt, result).await
            }
        }
    }

    pub(crate) async fn update_profile_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        locale: &str,
        seller_id: Uuid,
        input: UpdateMarketplaceSellerProfileInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let locale = normalize_seller_locale(locale)?;
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
            "locale": locale,
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
                    if legal_name_present {
                        active.legal_name = Set(legal_name.clone());
                    }
                    if let Some(metadata) = metadata.clone() {
                        active.metadata = Set(metadata);
                    }
                    active.updated_at = Set(Utc::now().into());
                    active.update(&receipt.transaction).await?;
                    if let Some(display_name) = display_name.clone() {
                        upsert_translation(
                            &receipt.transaction,
                            tenant_id,
                            seller_id,
                            locale.as_str(),
                            display_name,
                        )
                        .await?;
                    }
                    load_seller_response(
                        &receipt.transaction,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                    )
                    .await
                }
                .await;
                finish_seller_command(receipt, result).await
            }
        }
    }

    pub(crate) async fn submit_onboarding_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        locale: &str,
        seller_id: Uuid,
        input: SubmitMarketplaceSellerOnboardingInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let locale = normalize_seller_locale(locale)?;
        let note = optional_text(input.note);
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash(
            "submit_seller_onboarding",
            actor_id,
            &serde_json::json!({"locale": locale, "seller_id": seller_id, "note": note}),
        )?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            "submit_seller_onboarding",
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                "submit_seller_onboarding",
                hash.as_str(),
                RESPONSE_KIND_SELLER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result: MarketplaceSellerResult<MarketplaceSellerResponse> = async {
                    let update = seller::Entity::update_many()
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
                        .filter(seller::Column::Status.eq(MarketplaceSellerStatus::Draft.as_str()))
                        .filter(seller::Column::OnboardingStatus.is_in([
                            MarketplaceSellerOnboardingStatus::Draft.as_str(),
                            MarketplaceSellerOnboardingStatus::Rejected.as_str(),
                        ]))
                        .exec(&receipt.transaction)
                        .await?;
                    require_transition(
                        &receipt.transaction,
                        update.rows_affected,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                        "submitted",
                    )
                    .await?;
                    load_seller_response(
                        &receipt.transaction,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                    )
                    .await
                }
                .await;
                finish_seller_command(receipt, result).await
            }
        }
    }

    pub(crate) async fn review_onboarding_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        locale: &str,
        seller_id: Uuid,
        input: ReviewMarketplaceSellerOnboardingInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let locale = normalize_seller_locale(locale)?;
        let approved = input.approved;
        let note = optional_text(input.note);
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash(
            "review_seller_onboarding",
            actor_id,
            &serde_json::json!({
                "locale": locale,
                "seller_id": seller_id,
                "approved": approved,
                "note": note,
            }),
        )?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            "review_seller_onboarding",
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                "review_seller_onboarding",
                hash.as_str(),
                RESPONSE_KIND_SELLER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result: MarketplaceSellerResult<MarketplaceSellerResponse> = async {
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
                        .filter(seller::Column::Status.eq(MarketplaceSellerStatus::Draft.as_str()))
                        .filter(
                            seller::Column::OnboardingStatus
                                .eq(MarketplaceSellerOnboardingStatus::Submitted.as_str()),
                        );
                    if approved {
                        update = update.col_expr(
                            seller::Column::ActivatedAt,
                            sea_orm::sea_query::Expr::value(Some(now)),
                        );
                    }
                    let update = update.exec(&receipt.transaction).await?;
                    require_transition(
                        &receipt.transaction,
                        update.rows_affected,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                        onboarding.as_str(),
                    )
                    .await?;
                    load_seller_response(
                        &receipt.transaction,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                    )
                    .await
                }
                .await;
                finish_seller_command(receipt, result).await
            }
        }
    }

    pub(crate) async fn suspend_seller_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        locale: &str,
        seller_id: Uuid,
        input: SuspendMarketplaceSellerInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        input
            .validate()
            .map_err(|error| MarketplaceSellerError::Validation(error.to_string()))?;
        let locale = normalize_seller_locale(locale)?;
        let reason = required_text(input.reason, "reason")?;
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash(
            "suspend_seller",
            actor_id,
            &serde_json::json!({"locale": locale, "seller_id": seller_id, "reason": reason}),
        )?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            "suspend_seller",
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                "suspend_seller",
                hash.as_str(),
                RESPONSE_KIND_SELLER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result: MarketplaceSellerResult<MarketplaceSellerResponse> = async {
                    let now = Utc::now().fixed_offset();
                    let update = seller::Entity::update_many()
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
                        .filter(seller::Column::Status.eq(MarketplaceSellerStatus::Active.as_str()))
                        .exec(&receipt.transaction)
                        .await?;
                    require_transition(
                        &receipt.transaction,
                        update.rows_affected,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                        "suspended",
                    )
                    .await?;
                    load_seller_response(
                        &receipt.transaction,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                    )
                    .await
                }
                .await;
                finish_seller_command(receipt, result).await
            }
        }
    }

    pub(crate) async fn reactivate_seller_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        locale: &str,
        seller_id: Uuid,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        let locale = normalize_seller_locale(locale)?;
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash(
            "reactivate_seller",
            actor_id,
            &serde_json::json!({"locale": locale, "seller_id": seller_id}),
        )?;
        match admit_command(
            self.database(),
            tenant_id,
            actor_id,
            key,
            "reactivate_seller",
            hash.as_str(),
        )
        .await?
        {
            CommandReceiptAdmission::Replay(receipt) => replay_command(
                receipt,
                "reactivate_seller",
                hash.as_str(),
                RESPONSE_KIND_SELLER,
            ),
            CommandReceiptAdmission::New(receipt) => {
                let result: MarketplaceSellerResult<MarketplaceSellerResponse> = async {
                    let update = seller::Entity::update_many()
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
                            seller::Column::Status.eq(MarketplaceSellerStatus::Suspended.as_str()),
                        )
                        .filter(
                            seller::Column::OnboardingStatus
                                .eq(MarketplaceSellerOnboardingStatus::Approved.as_str()),
                        )
                        .exec(&receipt.transaction)
                        .await?;
                    require_transition(
                        &receipt.transaction,
                        update.rows_affected,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                        "active",
                    )
                    .await?;
                    load_seller_response(
                        &receipt.transaction,
                        tenant_id,
                        seller_id,
                        locale.as_str(),
                    )
                    .await
                }
                .await;
                finish_seller_command(receipt, result).await
            }
        }
    }

    pub(crate) async fn add_member_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        locale: &str,
        seller_id: Uuid,
        input: AddMarketplaceSellerMemberInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerMemberResponse> {
        let locale = normalize_seller_locale(locale)?;
        let metadata = object_or_empty(input.metadata, "metadata")?;
        let normalized = serde_json::json!({
            "locale": locale,
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
                        status: Set(MarketplaceSellerMemberStatus::Invited.as_str().to_string()),
                        invited_by_actor_id: Set(Some(actor_id)),
                        accepted_at: Set(None),
                        metadata: Set(metadata),
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
                finish_member_command(receipt, locale.as_str(), result).await
            }
        }
    }

    pub(crate) async fn update_member_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        locale: &str,
        seller_id: Uuid,
        member_id: Uuid,
        input: UpdateMarketplaceSellerMemberInput,
    ) -> MarketplaceSellerResult<MarketplaceSellerMemberResponse> {
        let locale = normalize_seller_locale(locale)?;
        let policy_input = input.clone();
        let metadata = input
            .metadata
            .map(|value| object_or_empty(value, "metadata"))
            .transpose()?;
        let normalized = serde_json::json!({
            "locale": locale,
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
                    validate_owner_membership_update(&current, &policy_input)?;
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
                    if let Some(metadata) = metadata {
                        active.metadata = Set(metadata);
                    }
                    active.updated_at = Set(Utc::now().into());
                    map_member(active.update(&receipt.transaction).await?)
                }
                .await;
                finish_member_command(receipt, locale.as_str(), result).await
            }
        }
    }
}

async fn require_transition(
    transaction: &sea_orm::DatabaseTransaction,
    rows_affected: u64,
    tenant_id: Uuid,
    seller_id: Uuid,
    locale: &str,
    to: &str,
) -> MarketplaceSellerResult<()> {
    if rows_affected == 1 {
        return Ok(());
    }
    let current = load_seller_response(transaction, tenant_id, seller_id, locale).await?;
    Err(MarketplaceSellerError::InvalidTransition {
        from: format!(
            "{}:{}",
            current.status.as_str(),
            current.onboarding_status.as_str()
        ),
        to: to.to_string(),
    })
}

async fn finish_seller_command(
    receipt: crate::command_receipts::NewCommandReceipt,
    result: MarketplaceSellerResult<MarketplaceSellerResponse>,
) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
    match result {
        Ok(response) => complete_command(receipt, RESPONSE_KIND_SELLER, &response).await,
        Err(error) => rollback_command(receipt, error).await,
    }
}

async fn finish_member_command(
    receipt: crate::command_receipts::NewCommandReceipt,
    locale: &str,
    result: MarketplaceSellerResult<MarketplaceSellerMemberResponse>,
) -> MarketplaceSellerResult<MarketplaceSellerMemberResponse> {
    match result {
        Ok(response) => {
            if let Err(error) = append_receipted_member_event(
                &receipt.transaction,
                receipt.tenant_id,
                receipt.actor_id,
                locale,
                receipt.command_kind.as_str(),
                &response,
            )
            .await
            {
                return rollback_command(receipt, error).await;
            }
            complete_command(receipt, RESPONSE_KIND_MEMBER, &response).await
        }
        Err(error) => rollback_command(receipt, error).await,
    }
}
