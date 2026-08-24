use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::idempotency::{self, Admission};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
    TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationResourceSummary, TranslationStrategy, TranslationTargetCapability,
    TranslationTargetChange, TranslationTargetChangePage, TranslationTargetChangesRequest,
    TranslationTargetProgressFacts, TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationValueProfile,
    provider_support::{
        contract_validation_error, decode_application_receipt, field_hash, merged_patch_values,
        opaque_positive_revision, parse_positive_revision, parse_resource_lifecycle,
        read_request_from_patch, required_target_value, validate_patch_against_snapshot,
        validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait};
use uuid::Uuid;

use crate::{
    MenuService, NavigationError,
    entities::{
        menu::{Column as MenuColumn, Entity as MenuEntity, Model as MenuModel},
        menu_item::{Column as MenuItemColumn, Entity as MenuItemEntity, Model as MenuItemModel},
        menu_item_translation::{
            Column as MenuItemTranslationColumn, Entity as MenuItemTranslationEntity,
            Model as MenuItemTranslationModel,
        },
        menu_translation::{
            Column as MenuTranslationColumn, Entity as MenuTranslationEntity,
            Model as MenuTranslationModel,
        },
        translation_change::{
            Column as TranslationChangeColumn, Entity as TranslationChangeEntity,
            Model as TranslationChangeModel,
        },
    },
    services::menu::ApplyExactMenuTranslationInput,
    translation_evidence::{TRANSLATION_OWNER_SLUG, TRANSLATION_RESOURCE_KIND},
};

const OPERATION_APPLY_PATCH: &str = "translation_target_apply_patch";
const MENU_NAME_FIELD_KEY: &str = "menu_name";
const ITEM_TITLE_FIELD_PREFIX: &str = "item:";
const ITEM_TITLE_FIELD_SUFFIX: &str = ":title";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
/// Owner adapter for exact Navigation menu localization. It keeps a menu name
/// and every item title in one atomic locale aggregate and calls the canonical
/// Navigation service rather than exposing Navigation tables to Translation.
pub struct NavigationMenuTranslationTargetProvider {
    service: Arc<MenuService>,
}

impl NavigationMenuTranslationTargetProvider {
    pub fn new(service: Arc<MenuService>) -> Self {
        Self { service }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Navigation owner slug must satisfy the target contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Navigation resource kind must satisfy the target contract"),
            display_name: "Navigation menu".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["navigation:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["navigation:update".to_string()]),
        }
    }

    async fn load_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        let menu_id = parse_identity(&request.identity)?;
        let menu = MenuEntity::find_by_id(menu_id)
            .filter(MenuColumn::TenantId.eq(tenant_id))
            .one(self.service.database())
            .await
            .map_err(navigation_database_error_to_port_error)?
            .ok_or_else(|| {
                PortError::not_found(
                    "navigation.translation_resource_not_found",
                    "Navigation menu translation resource was not found",
                )
            })?;
        let aggregate = self
            .load_menu_aggregates(tenant_id, vec![menu])
            .await?
            .pop()
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "navigation.translation_aggregate_missing",
                    "Navigation menu aggregate disappeared while it was being loaded",
                )
            })?;
        snapshot_from_aggregate(aggregate, request)
    }

    async fn load_menu_aggregates(
        &self,
        tenant_id: Uuid,
        menus: Vec<MenuModel>,
    ) -> Result<Vec<MenuAggregate>, PortError> {
        if menus.is_empty() {
            return Ok(Vec::new());
        }

        let menu_ids = menus.iter().map(|menu| menu.id).collect::<Vec<_>>();
        let translations = MenuTranslationEntity::find()
            .filter(MenuTranslationColumn::TenantId.eq(tenant_id))
            .filter(MenuTranslationColumn::MenuId.is_in(menu_ids.clone()))
            .order_by_asc(MenuTranslationColumn::MenuId)
            .order_by_asc(MenuTranslationColumn::Locale)
            .all(self.service.database())
            .await
            .map_err(navigation_database_error_to_port_error)?;
        let items = MenuItemEntity::find()
            .filter(MenuItemColumn::TenantId.eq(tenant_id))
            .filter(MenuItemColumn::MenuId.is_in(menu_ids.clone()))
            .order_by_asc(MenuItemColumn::MenuId)
            .order_by_asc(MenuItemColumn::Position)
            .order_by_asc(MenuItemColumn::CreatedAt)
            .order_by_asc(MenuItemColumn::Id)
            .all(self.service.database())
            .await
            .map_err(navigation_database_error_to_port_error)?;
        let item_translations = MenuItemTranslationEntity::find()
            .filter(MenuItemTranslationColumn::TenantId.eq(tenant_id))
            .filter(MenuItemTranslationColumn::MenuId.is_in(menu_ids))
            .order_by_asc(MenuItemTranslationColumn::MenuId)
            .order_by_asc(MenuItemTranslationColumn::MenuItemId)
            .order_by_asc(MenuItemTranslationColumn::Locale)
            .all(self.service.database())
            .await
            .map_err(navigation_database_error_to_port_error)?;

        let mut translations_by_menu = BTreeMap::<Uuid, Vec<MenuTranslationModel>>::new();
        for translation in translations {
            translations_by_menu
                .entry(translation.menu_id)
                .or_default()
                .push(translation);
        }
        let mut items_by_menu = BTreeMap::<Uuid, Vec<MenuItemModel>>::new();
        for item in items {
            items_by_menu.entry(item.menu_id).or_default().push(item);
        }
        let mut item_translations_by_menu = BTreeMap::<Uuid, Vec<MenuItemTranslationModel>>::new();
        for translation in item_translations {
            item_translations_by_menu
                .entry(translation.menu_id)
                .or_default()
                .push(translation);
        }

        Ok(menus
            .into_iter()
            .map(|menu| MenuAggregate {
                menu_translations: translations_by_menu.remove(&menu.id).unwrap_or_default(),
                items: items_by_menu.remove(&menu.id).unwrap_or_default(),
                item_translations: item_translations_by_menu
                    .remove(&menu.id)
                    .unwrap_or_default(),
                menu,
            })
            .collect())
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &PortError) {
        if let Err(receipt_error) = idempotency::fail(self.service.database(), lease, error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Navigation translation-target failure receipt"
            );
        }
    }

    async fn latest_change_cursor(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<OpaqueCursor>, PortError> {
        TranslationChangeEntity::find()
            .filter(TranslationChangeColumn::TenantId.eq(tenant_id))
            .filter(TranslationChangeColumn::ResourceKind.eq(TRANSLATION_RESOURCE_KIND))
            .order_by_desc(TranslationChangeColumn::Id)
            .one(self.service.database())
            .await
            .map_err(navigation_database_error_to_port_error)?
            .map(|change| {
                OpaqueCursor::new(change.id.to_string()).map_err(|error| {
                    PortError::invariant_violation(
                        "navigation.translation_change_cursor_invalid",
                        error.to_string(),
                    )
                })
            })
            .transpose()
    }

    async fn progress_facts(
        &self,
        tenant_id: Uuid,
        request: &TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        let menus = MenuEntity::find()
            .inner_join(MenuTranslationEntity)
            .filter(MenuColumn::TenantId.eq(tenant_id))
            .filter(MenuTranslationColumn::TenantId.eq(tenant_id))
            .filter(MenuTranslationColumn::Locale.eq(request.source_locale.as_str()))
            .order_by_asc(MenuColumn::Id)
            .all(self.service.database())
            .await
            .map_err(navigation_database_error_to_port_error)?;
        let aggregates = self.load_menu_aggregates(tenant_id, menus).await?;

        let mut required_units = 0_u64;
        let mut exact_required_units = 0_u64;
        let mut resources = 0_u64;
        let mut complete_resources = 0_u64;
        for aggregate in aggregates {
            let source = locale_rows(&aggregate, &request.source_locale)?.ok_or_else(|| {
                PortError::invariant_violation(
                    "navigation.translation_source_missing",
                    "Navigation menu was selected without its exact source locale aggregate",
                )
            })?;
            let field_count = u64::try_from(aggregate.items.len())
                .map_err(|_| {
                    PortError::invariant_violation(
                        "navigation.translation_progress_overflow",
                        "Navigation menu item count exceeds the progress contract",
                    )
                })?
                .checked_add(1)
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "navigation.translation_progress_overflow",
                        "Navigation required progress count overflow",
                    )
                })?;
            required_units = required_units.checked_add(field_count).ok_or_else(|| {
                PortError::invariant_violation(
                    "navigation.translation_progress_overflow",
                    "Navigation required progress count overflow",
                )
            })?;
            resources = resources.checked_add(1).ok_or_else(|| {
                PortError::invariant_violation(
                    "navigation.translation_progress_overflow",
                    "Navigation resource count overflow",
                )
            })?;

            let Some(target) = locale_rows(&aggregate, &request.target_locale)? else {
                continue;
            };
            let mut translated = u64::from(!target.menu_translation.name.trim().is_empty());
            let mut complete = translated == 1;
            for item in &aggregate.items {
                let title = target.item_translations.get(&item.id).ok_or_else(|| {
                    PortError::invariant_violation(
                        "navigation.translation_target_missing_item",
                        "Navigation target aggregate omitted a menu item title",
                    )
                })?;
                let has_title = !title.title.trim().is_empty();
                translated = translated
                    .checked_add(u64::from(has_title))
                    .ok_or_else(|| {
                        PortError::invariant_violation(
                            "navigation.translation_progress_overflow",
                            "Navigation exact progress count overflow",
                        )
                    })?;
                complete &= has_title;
            }
            exact_required_units =
                exact_required_units
                    .checked_add(translated)
                    .ok_or_else(|| {
                        PortError::invariant_violation(
                            "navigation.translation_progress_overflow",
                            "Navigation exact progress count overflow",
                        )
                    })?;
            if complete {
                complete_resources = complete_resources.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "navigation.translation_progress_overflow",
                        "Navigation complete resource count overflow",
                    )
                })?;
            }

            if source.menu_translation.name.trim().is_empty() {
                return Err(PortError::invariant_violation(
                    "navigation.translation_source_invalid",
                    "Navigation source aggregate has an empty menu name",
                ));
            }
        }

        Ok(TranslationTargetProgressFacts {
            required_units,
            exact_required_units,
            optional_units: 0,
            exact_optional_units: 0,
            resources,
            complete_resources,
            owner_change_cursor: None,
        })
    }
}

#[async_trait]
impl TranslationTargetProvider for NavigationMenuTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        Self::descriptor_value()
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let after = request
            .cursor
            .as_ref()
            .map(|cursor| {
                Uuid::parse_str(cursor.as_str()).map_err(|_| {
                    PortError::validation(
                        "navigation.translation_cursor_invalid",
                        "Navigation translation cursor must be a menu UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = MenuEntity::find()
            .inner_join(MenuTranslationEntity)
            .filter(MenuColumn::TenantId.eq(tenant_id))
            .filter(MenuTranslationColumn::TenantId.eq(tenant_id))
            .filter(MenuTranslationColumn::Locale.eq(request.source_locale.as_str()))
            .order_by_asc(MenuColumn::Id);
        if let Some(after) = after {
            query = query.filter(MenuColumn::Id.gt(after));
        }
        let mut menus = query
            .limit(u64::from(request.limit) + 1)
            .all(self.service.database())
            .await
            .map_err(navigation_database_error_to_port_error)?;
        let has_more = menus.len() > usize::from(request.limit);
        if has_more {
            menus.truncate(usize::from(request.limit));
        }
        let next_cursor = has_more.then(|| menus.last()).flatten().map(|menu| {
            OpaqueCursor::new(menu.id.to_string())
                .expect("Navigation UUID cursor must satisfy the opaque cursor contract")
        });
        let resources = self
            .load_menu_aggregates(tenant_id, menus)
            .await?
            .iter()
            .map(|aggregate| summary_from_aggregate(aggregate, &request.source_locale))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TranslationResourcePage {
            resources,
            next_cursor,
        })
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        if request.source_locale == request.target_locale {
            return Err(PortError::validation(
                "translation.equal_source_target_locale",
                "source and target locale must differ",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        self.load_snapshot(tenant_id, &request).await
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let snapshot = self
            .load_snapshot(tenant_id, &read_request_from_patch(&request))
            .await?;
        Ok(validate_patch_against_snapshot(&request, &snapshot))
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        validate_translation_apply_context(&context)?;
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let menu_id = parse_identity(&request.identity)?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        let lease = match idempotency::admit(
            self.service.database(),
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            TRANSLATION_OWNER_SLUG,
            idempotency_key,
            OPERATION_APPLY_PATCH,
            &request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => return decode_application_receipt(value),
            Admission::ReplayError(error) => return Err(error),
        };

        let result = async {
            let snapshot = self
                .load_snapshot(tenant_id, &read_request_from_patch(&request))
                .await?;
            let validation = validate_patch_against_snapshot(&request, &snapshot);
            if !validation.accepted {
                return Err(validation_to_port_error(&validation));
            }
            let target = merged_target(&request, &snapshot)?;
            let transaction = self
                .service
                .database()
                .begin()
                .await
                .map_err(navigation_database_error_to_port_error)?;
            let applied = self
                .service
                .apply_exact_translation_in_tx(
                    &transaction,
                    tenant_id,
                    menu_id,
                    ApplyExactMenuTranslationInput {
                        source_locale: request.source_locale.clone(),
                        target_locale: request.target_locale.clone(),
                        name: target.name,
                        item_titles: target.item_titles,
                        expected_resource_revision: parse_positive_revision(
                            &request.expected_resource_revision,
                            "expected_resource_revision",
                        )?,
                        expected_source_revision: parse_positive_revision(
                            &request.expected_source_revision,
                            "expected_source_revision",
                        )?,
                        expected_target_revision: request
                            .expected_target_revision
                            .as_ref()
                            .map(|revision| {
                                parse_positive_revision(revision, "expected_target_revision")
                            })
                            .transpose()?,
                    },
                )
                .await
                .map_err(navigation_error_to_port_error)?;
            let receipt = TranslationApplicationReceipt {
                provider_receipt_id: format!("navigation:{}", lease.operation_id),
                resource_revision: opaque_positive_revision(
                    applied.resource_revision,
                    "resource_revision",
                )?,
                target_revision: opaque_positive_revision(
                    applied.target_revision,
                    "target_revision",
                )?,
                applied_field_keys: request
                    .fields
                    .iter()
                    .map(|field| field.key.clone())
                    .collect(),
            };
            idempotency::complete(&transaction, lease, &receipt).await?;
            transaction
                .commit()
                .await
                .map_err(navigation_database_error_to_port_error)?;
            Ok(receipt)
        }
        .await;

        if let Err(error) = &result {
            self.fail_receipt(lease, error).await;
        }
        result
    }

    async fn read_progress(
        &self,
        context: PortContext,
        request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;

        for _ in 0..PROGRESS_STABILITY_ATTEMPTS {
            let cursor_before = self.latest_change_cursor(tenant_id).await?;
            let mut facts = self.progress_facts(tenant_id, &request).await?;
            let cursor_after = self.latest_change_cursor(tenant_id).await?;
            if cursor_before == cursor_after {
                facts.owner_change_cursor = cursor_after;
                facts.validate().map_err(|error| {
                    PortError::invariant_violation(
                        "navigation.translation_progress_invalid",
                        error.to_string(),
                    )
                })?;
                return Ok(facts);
            }
        }

        Err(PortError::unavailable(
            "navigation.translation_progress_unstable",
            "Navigation translation progress changed while it was being aggregated",
        ))
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let after = request
            .after
            .as_ref()
            .map(|cursor| {
                Uuid::parse_str(cursor.as_str()).map_err(|_| {
                    PortError::validation(
                        "navigation.translation_change_cursor_invalid",
                        "Navigation translation change cursor must be a change UUID",
                    )
                })
            })
            .transpose()?;
        let mut query = TranslationChangeEntity::find()
            .filter(TranslationChangeColumn::TenantId.eq(tenant_id))
            .filter(TranslationChangeColumn::ResourceKind.eq(TRANSLATION_RESOURCE_KIND))
            .order_by_asc(TranslationChangeColumn::Id);
        if let Some(after) = after {
            query = query.filter(TranslationChangeColumn::Id.gt(after));
        }
        let rows = query
            .limit(u64::from(request.limit))
            .all(self.service.database())
            .await
            .map_err(navigation_database_error_to_port_error)?;
        let next_cursor = rows.last().map(|change| {
            OpaqueCursor::new(change.id.to_string())
                .expect("Navigation change UUID must satisfy the opaque cursor contract")
        });
        let changes = rows
            .into_iter()
            .map(change_from_model)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TranslationTargetChangePage {
            changes,
            next_cursor,
        })
    }
}

struct MenuAggregate {
    menu: MenuModel,
    menu_translations: Vec<MenuTranslationModel>,
    items: Vec<MenuItemModel>,
    item_translations: Vec<MenuItemTranslationModel>,
}

struct MenuLocaleRows<'a> {
    menu_translation: &'a MenuTranslationModel,
    item_translations: BTreeMap<Uuid, &'a MenuItemTranslationModel>,
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "navigation.invalid_tenant_id",
            "Navigation translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<(), PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Navigation, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "navigation.translation_permission_denied",
            format!("navigation:{action} permission is required"),
        ));
    }
    Ok(())
}

fn parse_identity(identity: &TranslationResourceIdentity) -> Result<Uuid, PortError> {
    if identity.owner_slug.as_str() != TRANSLATION_OWNER_SLUG
        || identity.resource_kind.as_str() != TRANSLATION_RESOURCE_KIND
        || identity.subresource_id.is_some()
    {
        return Err(PortError::validation(
            "navigation.translation_identity_invalid",
            "Navigation translation identity must address navigation/menu without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "navigation.translation_resource_id_invalid",
            "Navigation menu translation resource id must be a UUID",
        )
    })
}

fn summary_from_aggregate(
    aggregate: &MenuAggregate,
    source_locale: &TenantLocale,
) -> Result<TranslationResourceSummary, PortError> {
    let source = locale_rows(aggregate, source_locale)?.ok_or_else(|| {
        PortError::invariant_violation(
            "navigation.translation_source_missing",
            "Navigation menu was listed without its exact source locale aggregate",
        )
    })?;
    Ok(TranslationResourceSummary {
        identity: menu_identity(aggregate.menu.id),
        display_label: source.menu_translation.name.clone(),
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_positive_revision(aggregate.menu.revision, "resource_revision")?,
        exact_locales: exact_locales(aggregate)?,
    })
}

fn snapshot_from_aggregate(
    aggregate: MenuAggregate,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    let source = locale_rows(&aggregate, &request.source_locale)?.ok_or_else(|| {
        PortError::not_found(
            "navigation.translation_source_not_found",
            "Exact source Navigation menu locale aggregate was not found",
        )
    })?;
    let target = locale_rows(&aggregate, &request.target_locale)?;
    let summary = summary_from_aggregate(&aggregate, &request.source_locale)?;
    let snapshot = TranslationResourceSnapshot {
        summary,
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: opaque_positive_revision(
            source.menu_translation.revision,
            "source_revision",
        )?,
        target_revision: target
            .as_ref()
            .map(|rows| opaque_positive_revision(rows.menu_translation.revision, "target_revision"))
            .transpose()?,
        fields: translation_fields(&aggregate.items, &source, target.as_ref())?,
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation("navigation.translation_snapshot_invalid", error.to_string())
    })?;
    Ok(snapshot)
}

fn locale_rows<'a>(
    aggregate: &'a MenuAggregate,
    locale: &TenantLocale,
) -> Result<Option<MenuLocaleRows<'a>>, PortError> {
    let mut menu_translations = aggregate
        .menu_translations
        .iter()
        .filter(|translation| translation.locale == locale.as_str());
    let Some(menu_translation) = menu_translations.next() else {
        if aggregate
            .item_translations
            .iter()
            .any(|translation| translation.locale == locale.as_str())
        {
            return Err(PortError::invariant_violation(
                "navigation.translation_locale_orphaned_items",
                "Navigation locale has item translations without a menu translation",
            ));
        }
        return Ok(None);
    };
    if menu_translations.next().is_some() {
        return Err(PortError::invariant_violation(
            "navigation.translation_locale_duplicate_menu",
            "Navigation locale has duplicate menu translations",
        ));
    }

    let expected_item_ids = aggregate
        .items
        .iter()
        .map(|item| item.id)
        .collect::<BTreeSet<_>>();
    let mut item_translations = BTreeMap::new();
    for translation in aggregate
        .item_translations
        .iter()
        .filter(|translation| translation.locale == locale.as_str())
    {
        if item_translations
            .insert(translation.menu_item_id, translation)
            .is_some()
        {
            return Err(PortError::invariant_violation(
                "navigation.translation_locale_duplicate_item",
                "Navigation locale has duplicate menu item translations",
            ));
        }
    }
    if item_translations.keys().copied().collect::<BTreeSet<_>>() != expected_item_ids {
        return Err(PortError::invariant_violation(
            "navigation.translation_locale_incomplete",
            "Navigation menu locale must cover every menu item exactly once",
        ));
    }

    Ok(Some(MenuLocaleRows {
        menu_translation,
        item_translations,
    }))
}

fn exact_locales(aggregate: &MenuAggregate) -> Result<Vec<TenantLocale>, PortError> {
    let locale_strings = aggregate
        .menu_translations
        .iter()
        .map(|translation| translation.locale.as_str())
        .chain(
            aggregate
                .item_translations
                .iter()
                .map(|translation| translation.locale.as_str()),
        )
        .collect::<BTreeSet<_>>();
    locale_strings
        .into_iter()
        .map(|locale| {
            let locale = TenantLocale::new(locale).map_err(|error| {
                PortError::invariant_violation(
                    "navigation.translation_locale_invalid",
                    error.to_string(),
                )
            })?;
            locale_rows(aggregate, &locale)?.ok_or_else(|| {
                PortError::invariant_violation(
                    "navigation.translation_locale_missing_menu",
                    "Navigation menu item locale has no menu translation",
                )
            })?;
            Ok(locale)
        })
        .collect()
}

fn menu_identity(menu_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Navigation owner slug must satisfy the target contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Navigation resource kind must satisfy the target contract"),
        resource_id: ResourceId::new(menu_id.to_string())
            .expect("Navigation menu UUID must satisfy the resource id contract"),
        subresource_id: None,
    }
}

fn translation_fields(
    items: &[MenuItemModel],
    source: &MenuLocaleRows<'_>,
    target: Option<&MenuLocaleRows<'_>>,
) -> Result<Vec<TranslationFieldSnapshot>, PortError> {
    let mut fields = Vec::with_capacity(items.len() + 1);
    fields.push(field_snapshot(
        FieldKey::new(MENU_NAME_FIELD_KEY).expect("static Navigation field key must be valid"),
        source.menu_translation.name.as_str(),
        target.map(|rows| rows.menu_translation.name.as_str()),
    ));
    for item in items {
        let source_title = source.item_translations.get(&item.id).ok_or_else(|| {
            PortError::invariant_violation(
                "navigation.translation_source_missing_item",
                "Navigation source aggregate omitted a menu item title",
            )
        })?;
        let target_title = target
            .map(|rows| {
                rows.item_translations.get(&item.id).ok_or_else(|| {
                    PortError::invariant_violation(
                        "navigation.translation_target_missing_item",
                        "Navigation target aggregate omitted a menu item title",
                    )
                })
            })
            .transpose()?;
        fields.push(field_snapshot(
            menu_item_title_field_key(item.id)?,
            source_title.title.as_str(),
            target_title.map(|translation| translation.title.as_str()),
        ));
    }
    Ok(fields)
}

fn field_snapshot(
    key: FieldKey,
    source_value: &str,
    target_value: Option<&str>,
) -> TranslationFieldSnapshot {
    TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key,
            profile: TranslationValueProfile::PlainText,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::Public,
            required: true,
            ai_export_allowed: true,
            max_characters: Some(255),
            preserves_whitespace: false,
        },
        source_value: source_value.to_string(),
        exact_target_value: target_value.map(str::to_string),
        source_hash: field_hash(source_value),
        protected_tokens: Vec::new(),
    }
}

fn menu_item_title_field_key(item_id: Uuid) -> Result<FieldKey, PortError> {
    FieldKey::new(format!(
        "{ITEM_TITLE_FIELD_PREFIX}{item_id}{ITEM_TITLE_FIELD_SUFFIX}"
    ))
    .map_err(|error| {
        PortError::invariant_violation(
            "navigation.translation_item_field_key_invalid",
            error.to_string(),
        )
    })
}

struct MergedTarget {
    name: String,
    item_titles: BTreeMap<Uuid, String>,
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<MergedTarget, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let name = required_target_value(
        values.remove(MENU_NAME_FIELD_KEY).flatten(),
        MENU_NAME_FIELD_KEY,
    )?;
    let mut item_titles = BTreeMap::new();
    for field in snapshot
        .fields
        .iter()
        .filter(|field| field.descriptor.key.as_str() != MENU_NAME_FIELD_KEY)
    {
        let item_id = parse_menu_item_title_field_key(field.descriptor.key.as_str())?;
        let title = required_target_value(
            values.remove(field.descriptor.key.as_str()).flatten(),
            field.descriptor.key.as_str(),
        )?;
        if item_titles.insert(item_id, title).is_some() {
            return Err(PortError::invariant_violation(
                "navigation.translation_item_field_duplicate",
                "Navigation translation snapshot exposed a duplicate menu item field",
            ));
        }
    }
    Ok(MergedTarget { name, item_titles })
}

fn parse_menu_item_title_field_key(key: &str) -> Result<Uuid, PortError> {
    let item_id = key
        .strip_prefix(ITEM_TITLE_FIELD_PREFIX)
        .and_then(|value| value.strip_suffix(ITEM_TITLE_FIELD_SUFFIX))
        .ok_or_else(|| {
            PortError::invariant_violation(
                "navigation.translation_item_field_key_invalid",
                "Navigation translation snapshot exposed an unsupported field key",
            )
        })?;
    Uuid::parse_str(item_id).map_err(|_| {
        PortError::invariant_violation(
            "navigation.translation_item_field_key_invalid",
            "Navigation translation snapshot exposed a non-UUID menu item field key",
        )
    })
}

fn change_from_model(change: TranslationChangeModel) -> Result<TranslationTargetChange, PortError> {
    Ok(TranslationTargetChange {
        identity: menu_identity(change.resource_id),
        resource_revision: opaque_positive_revision(change.resource_revision, "resource_revision")?,
        lifecycle: parse_resource_lifecycle(&change.lifecycle)?,
    })
}

fn navigation_error_to_port_error(error: NavigationError) -> PortError {
    match error {
        NavigationError::MenuNotFound(_) => PortError::not_found(
            "navigation.translation_resource_not_found",
            "Navigation menu translation resource was not found",
        ),
        NavigationError::Conflict(_) => PortError::conflict(
            "navigation.translation_owner_conflict",
            "Navigation state conflicts with the requested translation mutation",
        ),
        NavigationError::Forbidden(_) => PortError::forbidden(
            "navigation.translation_permission_denied",
            "Navigation permission is required",
        ),
        NavigationError::Validation(_) => PortError::validation(
            "navigation.translation_owner_validation",
            "Navigation rejected the translation mutation",
        ),
        NavigationError::TranslationRevisionExhausted { .. } => PortError::invariant_violation(
            "navigation.translation_revision_exhausted",
            "Navigation translation revision is exhausted",
        ),
        NavigationError::Database(_) | NavigationError::Core(_) | NavigationError::Rich(_) => {
            PortError::unavailable(
                "navigation.translation_owner_unavailable",
                "Navigation translation storage is unavailable",
            )
        }
    }
}

fn navigation_database_error_to_port_error(error: sea_orm::DbErr) -> PortError {
    navigation_error_to_port_error(NavigationError::Database(error))
}
