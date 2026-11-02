use rustok_core::error::Error as CoreError;
use thiserror::Error;
use uuid::Uuid;

/// Errors owned by the Product domain.
#[derive(Error, Debug)]
pub enum CommerceError {
    #[error("Product storage error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Product not found: {0}")]
    ProductNotFound(Uuid),

    #[error("Duplicate handle: {handle} already exists for locale {locale}")]
    DuplicateHandle { handle: String, locale: String },

    #[error("Duplicate SKU: {0}")]
    DuplicateSku(String),

    #[error("Product validation error: {0}")]
    Validation(String),

    #[error("{0}")]
    VariantAxisPolicyConflict(crate::dto::VariantAxisPolicyConflict),

    #[error("Product state conflict: {0}")]
    Conflict(String),

    #[error("Product must have at least one variant")]
    NoVariants,

    #[error("Product variant not found: {0}")]
    VariantNotFound(Uuid),

    #[error("Product image not found: {0}")]
    ImageNotFound(Uuid),

    #[error("Cannot delete the only variant of a product")]
    CannotDeleteOnlyVariant,

    #[error("Cannot delete published product")]
    CannotDeletePublished,

    #[error("Inventory prevents catalog deletion: {0}")]
    InventoryDeletionBlocked(rustok_inventory::CatalogDeletionBlocker),

    #[error("Inventory prevents quantity update: {0}")]
    InventoryQuantityBlocked(rustok_inventory::QuantityWriteBlocker),

    #[error("Product core operation failed: {0}")]
    Core(#[from] CoreError),
}

pub type CommerceResult<T> = Result<T, CommerceError>;

/// Preserve the typed Inventory refusal without rendering dependency/database details.
impl From<rustok_inventory::CatalogDeletionError> for CommerceError {
    fn from(error: rustok_inventory::CatalogDeletionError) -> Self {
        match error {
            rustok_inventory::CatalogDeletionError::Database(error) => Self::Database(error),
            rustok_inventory::CatalogDeletionError::Rejected(blocker) => {
                Self::InventoryDeletionBlocked(blocker)
            }
        }
    }
}

fn inventory_deletion_error_policy(
    blocker: &rustok_inventory::CatalogDeletionBlocker,
) -> (rustok_api::PortErrorKind, &'static str, &'static str) {
    use rustok_api::PortErrorKind;
    use rustok_inventory::CatalogDeletionBlocker;
    match blocker {
        CatalogDeletionBlocker::InvalidRequest => (
            PortErrorKind::Validation,
            "product.inventory_deletion_invalid",
            "PRODUCT_INVENTORY_DELETION_INVALID",
        ),
        CatalogDeletionBlocker::LimitExceeded => (
            PortErrorKind::Validation,
            "product.inventory_deletion_limit",
            "PRODUCT_INVENTORY_DELETION_LIMIT",
        ),
        CatalogDeletionBlocker::IdentityUnavailable => (
            PortErrorKind::InvariantViolation,
            "product.inventory_deletion_identity_unavailable",
            "PRODUCT_INVENTORY_DELETION_IDENTITY_UNAVAILABLE",
        ),
        CatalogDeletionBlocker::StateUnverified => (
            PortErrorKind::InvariantViolation,
            "product.inventory_deletion_state_unverified",
            "PRODUCT_INVENTORY_DELETION_STATE_UNVERIFIED",
        ),
        CatalogDeletionBlocker::IsolationUnsupported => (
            PortErrorKind::InvariantViolation,
            "product.inventory_deletion_isolation_unsupported",
            "PRODUCT_INVENTORY_DELETION_ISOLATION_UNSUPPORTED",
        ),
        CatalogDeletionBlocker::HistoryPresent => (
            PortErrorKind::Conflict,
            "product.inventory_deletion_history_present",
            "PRODUCT_INVENTORY_DELETION_HISTORY_PRESENT",
        ),
        CatalogDeletionBlocker::StockPresent => (
            PortErrorKind::Conflict,
            "product.inventory_deletion_stock_present",
            "PRODUCT_INVENTORY_DELETION_STOCK_PRESENT",
        ),
        CatalogDeletionBlocker::ConcurrentChange => (
            PortErrorKind::Conflict,
            "product.inventory_deletion_changed",
            "PRODUCT_INVENTORY_DELETION_CHANGED",
        ),
    }
}

/// Safe finite Inventory-deletion classification for Product transport adapters.
pub fn inventory_deletion_port_error(
    blocker: &rustok_inventory::CatalogDeletionBlocker,
) -> rustok_api::PortError {
    let (kind, code, _) = inventory_deletion_error_policy(blocker);
    rustok_api::PortError::new(kind, code, blocker.message(), false)
}

pub(crate) fn inventory_deletion_error_code(
    blocker: &rustok_inventory::CatalogDeletionBlocker,
) -> &'static str {
    inventory_deletion_error_policy(blocker).1
}

/// Fixed public copy/code for a typed owner refusal, with no dependency rendering.
pub fn inventory_deletion_public_policy(
    blocker: &rustok_inventory::CatalogDeletionBlocker,
) -> (&'static str, &'static str, bool) {
    let (_, _, code) = inventory_deletion_error_policy(blocker);
    (blocker.message(), code, false)
}

/// Reconstruct known refusal copy from a closed owner code and its expected kind only.
/// Provider messages and retry advice are not public copy, receipts or no-effect proof.
pub fn inventory_deletion_public_failure(
    error: &rustok_api::PortError,
) -> Option<(&'static str, &'static str, bool)> {
    inventory_deletion_blocker(error)
        .as_ref()
        .map(inventory_deletion_public_policy)
}

pub(crate) fn inventory_deletion_blocker(
    error: &rustok_api::PortError,
) -> Option<rustok_inventory::CatalogDeletionBlocker> {
    rustok_inventory::CatalogDeletionBlocker::ALL
        .into_iter()
        .find(|blocker| {
            let (kind, code, _) = inventory_deletion_error_policy(blocker);
            error.kind == kind && error.code == code
        })
}

/// Preserve owner storage/unknown classes; publication validation is never user validation.
impl From<rustok_inventory::QuantityWriteError> for CommerceError {
    fn from(error: rustok_inventory::QuantityWriteError) -> Self {
        use rustok_inventory::{QuantityWriteBlocker, QuantityWriteError};
        match error {
            QuantityWriteError::Rejected(blocker) => Self::InventoryQuantityBlocked(blocker),
            QuantityWriteError::VariantNotFound(id) => Self::VariantNotFound(id),
            QuantityWriteError::Database(error)
            | QuantityWriteError::Publication(CoreError::Database(error)) => Self::Database(error),
            QuantityWriteError::Publication(_) => {
                Self::InventoryQuantityBlocked(QuantityWriteBlocker::PublicationUnverified)
            }
        }
    }
}

fn inventory_quantity_error_policy(
    blocker: &rustok_inventory::QuantityWriteBlocker,
) -> (rustok_api::PortErrorKind, &'static str, &'static str) {
    use rustok_inventory::QuantityWriteBlocker;
    let (code, public_code) = match blocker {
        QuantityWriteBlocker::InvalidRequest => (
            "product.inventory_quantity_invalid",
            "PRODUCT_INVENTORY_QUANTITY_INVALID",
        ),
        QuantityWriteBlocker::LimitExceeded => (
            "product.inventory_quantity_limit",
            "PRODUCT_INVENTORY_QUANTITY_LIMIT",
        ),
        QuantityWriteBlocker::IdentityUnavailable => (
            "product.inventory_quantity_identity_unavailable",
            "PRODUCT_INVENTORY_QUANTITY_IDENTITY_UNAVAILABLE",
        ),
        QuantityWriteBlocker::StateUnverified => (
            "product.inventory_quantity_state_unverified",
            "PRODUCT_INVENTORY_QUANTITY_STATE_UNVERIFIED",
        ),
        QuantityWriteBlocker::LocationUnavailable => (
            "product.inventory_quantity_location_unavailable",
            "PRODUCT_INVENTORY_QUANTITY_LOCATION_UNAVAILABLE",
        ),
        QuantityWriteBlocker::QuantityUnrepresentable => (
            "product.inventory_quantity_unrepresentable",
            "PRODUCT_INVENTORY_QUANTITY_UNREPRESENTABLE",
        ),
        QuantityWriteBlocker::InsufficientStock => (
            "product.inventory_quantity_insufficient",
            "PRODUCT_INVENTORY_QUANTITY_INSUFFICIENT",
        ),
        QuantityWriteBlocker::ConcurrentChange => (
            "product.inventory_quantity_changed",
            "PRODUCT_INVENTORY_QUANTITY_CHANGED",
        ),
        QuantityWriteBlocker::IsolationUnsupported => (
            "product.inventory_quantity_isolation_unsupported",
            "PRODUCT_INVENTORY_QUANTITY_ISOLATION_UNSUPPORTED",
        ),
        QuantityWriteBlocker::EventUnrepresentable => (
            "product.inventory_quantity_event_unrepresentable",
            "PRODUCT_INVENTORY_QUANTITY_EVENT_UNREPRESENTABLE",
        ),
        QuantityWriteBlocker::PublicationUnverified => (
            "product.inventory_quantity_publication_unverified",
            "PRODUCT_INVENTORY_QUANTITY_PUBLICATION_UNVERIFIED",
        ),
    };
    (blocker.kind(), code, public_code)
}

pub fn inventory_quantity_port_error(
    blocker: &rustok_inventory::QuantityWriteBlocker,
) -> rustok_api::PortError {
    let (kind, code, _) = inventory_quantity_error_policy(blocker);
    rustok_api::PortError::new(kind, code, blocker.message(), false)
}

pub(crate) fn inventory_quantity_error_code(
    blocker: &rustok_inventory::QuantityWriteBlocker,
) -> &'static str {
    inventory_quantity_error_policy(blocker).1
}

pub fn inventory_quantity_public_policy(
    blocker: &rustok_inventory::QuantityWriteBlocker,
) -> (&'static str, &'static str, bool) {
    let (_, _, code) = inventory_quantity_error_policy(blocker);
    (blocker.message(), code, false)
}

/// Provider text and retry flags do not classify an owner refusal or certify its outcome.
pub fn inventory_quantity_public_failure(
    error: &rustok_api::PortError,
) -> Option<(&'static str, &'static str, bool)> {
    inventory_quantity_blocker(error)
        .as_ref()
        .map(inventory_quantity_public_policy)
}

pub(crate) fn inventory_quantity_blocker(
    error: &rustok_api::PortError,
) -> Option<rustok_inventory::QuantityWriteBlocker> {
    rustok_inventory::QuantityWriteBlocker::ALL
        .into_iter()
        .find(|blocker| {
            let (kind, code, _) = inventory_quantity_error_policy(blocker);
            error.kind == kind && error.code == code
        })
}

/// Flex attached values are an owner dependency, not a source of public error copy.
/// Keep this match exhaustive so additions to Flex require an explicit Product policy.
impl From<rustok_core::field_schema::FlexError> for CommerceError {
    fn from(error: rustok_core::field_schema::FlexError) -> Self {
        use rustok_core::field_schema::FlexError;
        match error {
            FlexError::Database(message) => Self::Database(sea_orm::DbErr::Custom(message)),
            // Product supplies a fixed attached entity type. Its rejection is owner/configuration
            // failure, not invalid user metadata, and must never become a terminal validation.
            FlexError::UnknownEntityType(_) => Self::Core(CoreError::Internal(
                "Product custom field entity registration is invalid".into(),
            )),
            // These describe the submitted custom-field payload/definition selection, not a
            // missing Product aggregate. Preserve the existing validation class, without data.
            FlexError::NotFound(_) => {
                Self::Validation("Custom field definition was not found".into())
            }
            FlexError::DuplicateFieldKey(_) => {
                Self::Validation("Custom field key already exists".into())
            }
            FlexError::InvalidLocale(_) => {
                Self::Validation("Custom field locale is invalid".into())
            }
            FlexError::InvalidFieldKey(_) => Self::Validation("Custom field key is invalid".into()),
            FlexError::TooManyFields { .. } => {
                Self::Validation("Custom field definition limit exceeded".into())
            }
            FlexError::ValidationFailed(_) => {
                Self::Validation("Custom field validation failed".into())
            }
        }
    }
}

/// Canonical classification of nested Core failures at Product boundaries.
/// No dependency message, source/debug rendering or submitted value is public copy.
fn core_error_policy(
    error: &CoreError,
) -> (rustok_api::PortErrorKind, &'static str, &'static str, bool) {
    use rustok_api::PortErrorKind;
    match error {
        CoreError::Database(_) => (
            PortErrorKind::Unavailable,
            "product.database_unavailable",
            "product storage is temporarily unavailable",
            true,
        ),
        CoreError::Cache(_) | CoreError::External(_) => (
            PortErrorKind::Unavailable,
            "product.dependency_unavailable",
            "product dependency is temporarily unavailable",
            true,
        ),
        CoreError::Auth(_) | CoreError::Forbidden(_) => (
            PortErrorKind::Forbidden,
            "product.forbidden",
            "product access is denied",
            false,
        ),
        CoreError::InvalidIdFormat(_) | CoreError::Validation(_) => (
            PortErrorKind::Validation,
            "product.validation",
            "product request is invalid",
            false,
        ),
        CoreError::NotFound(_) => (
            PortErrorKind::NotFound,
            "product.dependency_not_found",
            "referenced product resource was not found",
            false,
        ),
        CoreError::Serialization(_) | CoreError::Scripting(_) | CoreError::Internal(_) => (
            PortErrorKind::InvariantViolation,
            "product.invariant_violation",
            "product operation could not be completed safely",
            false,
        ),
    }
}

pub fn core_port_error(error: &CoreError) -> rustok_api::PortError {
    let (kind, code, message, retryable) = core_error_policy(error);
    rustok_api::PortError::new(kind, code, message, retryable)
}

pub fn core_error_code(error: &CoreError) -> &'static str {
    core_error_policy(error).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::field_schema::FlexError;

    #[test]
    fn nested_core_classes_are_explicit_and_never_render_dependency_details() {
        use rustok_api::PortErrorKind as K;
        let serialization = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let cases = [
            (
                CoreError::Database(sea_orm::DbErr::Custom("private-marker".into())),
                K::Unavailable,
                true,
            ),
            (
                CoreError::Serialization(serialization),
                K::InvariantViolation,
                false,
            ),
            (
                CoreError::Auth("private-marker".into()),
                K::Forbidden,
                false,
            ),
            (
                CoreError::Forbidden("private-marker".into()),
                K::Forbidden,
                false,
            ),
            (
                CoreError::NotFound("private-marker".into()),
                K::NotFound,
                false,
            ),
            (
                CoreError::Cache("private-marker".into()),
                K::Unavailable,
                true,
            ),
            (
                CoreError::External("private-marker".into()),
                K::Unavailable,
                true,
            ),
            (
                CoreError::InvalidIdFormat("private-marker".into()),
                K::Validation,
                false,
            ),
            (
                CoreError::Validation("private-marker".into()),
                K::Validation,
                false,
            ),
            (
                CoreError::Scripting("private-marker".into()),
                K::InvariantViolation,
                false,
            ),
            (
                CoreError::Internal("private-marker".into()),
                K::InvariantViolation,
                false,
            ),
        ];
        for (error, kind, retryable) in cases {
            let port = core_port_error(&error);
            assert_eq!(port.kind, kind);
            assert_eq!(port.retryable, retryable);
            assert_eq!(port.code, core_error_code(&error));
            assert!(!port.message.contains("private-marker"));
            let public = crate::map_product_public_error(
                &CommerceError::Core(error),
                "test",
                "product_core",
            );
            assert_eq!(public.retryable, retryable);
            assert!(!public.to_string().contains("private-marker"));
        }
    }

    #[test]
    fn flex_storage_and_fixed_entity_failures_are_not_client_validation() {
        assert!(matches!(
            CommerceError::from(FlexError::Database("private database detail".into())),
            CommerceError::Database(_)
        ));
        let registration =
            CommerceError::from(FlexError::UnknownEntityType("private entity detail".into()));
        assert!(matches!(
            &registration,
            CommerceError::Core(CoreError::Internal(_))
        ));
        assert!(!registration.to_string().contains("private entity detail"));
    }

    #[test]
    fn flex_payload_errors_have_explicit_static_copy_without_tenant_text() {
        let id = Uuid::new_v4();
        let cases = [
            FlexError::NotFound(id),
            FlexError::DuplicateFieldKey("private field detail".into()),
            FlexError::InvalidLocale("private locale detail".into()),
            FlexError::InvalidFieldKey("private field detail".into()),
            FlexError::TooManyFields {
                entity_type: "private entity detail".into(),
                max: 99,
            },
            FlexError::ValidationFailed(vec![]),
        ];
        for error in cases {
            let mapped = CommerceError::from(error);
            assert!(matches!(&mapped, CommerceError::Validation(_)));
            let rendered = mapped.to_string();
            assert!(!rendered.contains("private"));
            assert!(!rendered.contains(&id.to_string()));
        }
    }

    #[test]
    fn inventory_deletion_refusals_have_closed_kind_code_and_public_copy() {
        use rustok_api::PortErrorKind as K;
        use rustok_inventory::CatalogDeletionBlocker as B;
        let cases = [
            (
                B::InvalidRequest,
                K::Validation,
                "product.inventory_deletion_invalid",
                "PRODUCT_INVENTORY_DELETION_INVALID",
            ),
            (
                B::LimitExceeded,
                K::Validation,
                "product.inventory_deletion_limit",
                "PRODUCT_INVENTORY_DELETION_LIMIT",
            ),
            (
                B::IdentityUnavailable,
                K::InvariantViolation,
                "product.inventory_deletion_identity_unavailable",
                "PRODUCT_INVENTORY_DELETION_IDENTITY_UNAVAILABLE",
            ),
            (
                B::StateUnverified,
                K::InvariantViolation,
                "product.inventory_deletion_state_unverified",
                "PRODUCT_INVENTORY_DELETION_STATE_UNVERIFIED",
            ),
            (
                B::IsolationUnsupported,
                K::InvariantViolation,
                "product.inventory_deletion_isolation_unsupported",
                "PRODUCT_INVENTORY_DELETION_ISOLATION_UNSUPPORTED",
            ),
            (
                B::HistoryPresent,
                K::Conflict,
                "product.inventory_deletion_history_present",
                "PRODUCT_INVENTORY_DELETION_HISTORY_PRESENT",
            ),
            (
                B::StockPresent,
                K::Conflict,
                "product.inventory_deletion_stock_present",
                "PRODUCT_INVENTORY_DELETION_STOCK_PRESENT",
            ),
            (
                B::ConcurrentChange,
                K::Conflict,
                "product.inventory_deletion_changed",
                "PRODUCT_INVENTORY_DELETION_CHANGED",
            ),
        ];
        assert_eq!(cases.len(), B::ALL.len());
        for (reason, kind, code, public_code) in cases {
            let mut port = inventory_deletion_port_error(&reason);
            assert_eq!(port.kind, kind);
            assert_eq!(port.code, code);
            assert!(!port.retryable);
            let public = crate::map_product_public_error(
                &CommerceError::InventoryDeletionBlocked(reason),
                "delete_product",
                "deletion_test",
            );
            assert_eq!(public.code, public_code);
            assert_eq!(public.message, reason.message());
            assert!(!public.retryable);
            assert_eq!(
                crate::product_catalog_command_error_is_terminal(&port),
                matches!(kind, K::Validation | K::Conflict),
                "only finalized domain receipt classes may be described as terminal",
            );
            port.message = "private-provider-details".into();
            port.retryable = true;
            assert_eq!(
                inventory_deletion_public_failure(&port),
                Some((reason.message(), public_code, false))
            );
            assert!(
                !crate::product_catalog_command_error_is_terminal(&port),
                "presentation is not an outcome receipt"
            );
        }
    }

    #[test]
    fn inventory_deletion_code_kind_mismatch_and_unknown_codes_are_not_trusted() {
        let original = inventory_deletion_port_error(
            &rustok_inventory::CatalogDeletionBlocker::HistoryPresent,
        );
        for kind in [
            rustok_api::PortErrorKind::Unavailable,
            rustok_api::PortErrorKind::Forbidden,
        ] {
            let mismatch = rustok_api::PortError::new(
                kind,
                original.code.clone(),
                "private-provider-details",
                false,
            );
            assert!(inventory_deletion_public_failure(&mismatch).is_none());
            assert!(!crate::product_catalog_command_error_is_terminal(&mismatch));
        }
        let unknown = rustok_api::PortError::conflict(
            "product.inventory_deletion_unknown",
            "private-provider-details",
        );
        assert!(inventory_deletion_public_failure(&unknown).is_none());
        assert!(!crate::product_catalog_command_error_is_terminal(&unknown));
    }

    #[test]
    fn inventory_deletion_storage_failure_remains_unavailable_not_a_domain_refusal() {
        let error = CommerceError::from(rustok_inventory::CatalogDeletionError::Database(
            sea_orm::DbErr::Custom("private-database-details".into()),
        ));
        assert!(matches!(&error, CommerceError::Database(_)));
        let public = crate::map_product_public_error(&error, "delete_product", "deletion_test");
        assert_eq!(public.code, "PRODUCT_TEMPORARILY_UNAVAILABLE");
        assert!(public.retryable);
        assert!(!public.to_string().contains("private-database-details"));
    }
}

#[cfg(test)]
mod quantity_errors {
    use super::*;
    use rustok_inventory::QuantityWriteBlocker as B;

    #[test]
    fn quantity_refusals_have_closed_owner_codes_public_copy_and_separate_terminal_classes() {
        for reason in B::ALL {
            let mut port = inventory_quantity_port_error(&reason);
            assert_eq!(port.kind, reason.kind());
            assert!(port.code.starts_with("product.inventory_quantity_"));
            let terminal = matches!(
                port.kind,
                rustok_api::PortErrorKind::Validation | rustok_api::PortErrorKind::Conflict
            );
            assert_eq!(
                crate::product_catalog_command_error_is_terminal(&port),
                terminal
            );
            let public = crate::map_product_public_error(
                &CommerceError::InventoryQuantityBlocked(reason),
                "update_variant",
                "quantity_test",
            );
            assert_eq!(public.message, reason.message());
            assert!(!public.retryable);
            assert_eq!(
                inventory_quantity_public_failure(&port),
                Some((public.message, public.code, false))
            );
            port.message = "private-provider-details".into();
            port.retryable = true;
            assert_eq!(
                inventory_quantity_public_failure(&port),
                Some((public.message, public.code, false))
            );
            assert!(!crate::product_catalog_command_error_is_terminal(&port));
        }
    }

    #[test]
    fn quantity_wrong_code_kind_or_provider_text_cannot_reconstruct_a_refusal() {
        let mut port = inventory_quantity_port_error(&B::InsufficientStock);
        port.kind = rustok_api::PortErrorKind::Unavailable;
        assert!(inventory_quantity_public_failure(&port).is_none());
        assert!(!crate::product_catalog_command_error_is_terminal(&port));
        port.kind = rustok_api::PortErrorKind::Conflict;
        port.code = "unknown-owner-code".into();
        port.message = B::InsufficientStock.message().into();
        assert!(inventory_quantity_public_failure(&port).is_none());
        assert!(!crate::product_catalog_command_error_is_terminal(&port));
    }

    #[test]
    fn quantity_publication_and_storage_errors_never_become_terminal_client_validation() {
        let publication = CommerceError::from(rustok_inventory::QuantityWriteError::Publication(
            CoreError::Validation("private-provider-details".into()),
        ));
        assert!(matches!(
            &publication,
            CommerceError::InventoryQuantityBlocked(B::PublicationUnverified)
        ));
        let port = inventory_quantity_port_error(&B::PublicationUnverified);
        assert!(!crate::product_catalog_command_error_is_terminal(&port));
        for error in [
            rustok_inventory::QuantityWriteError::Database(sea_orm::DbErr::Custom(
                "private-provider-details".into(),
            )),
            rustok_inventory::QuantityWriteError::Publication(CoreError::Database(
                sea_orm::DbErr::Custom("private-provider-details".into()),
            )),
        ] {
            let mapped = CommerceError::from(error);
            assert!(matches!(&mapped, CommerceError::Database(_)));
            let public =
                crate::map_product_public_error(&mapped, "update_variant", "quantity_test");
            assert!(public.retryable);
            assert!(!public.message.contains("private-provider-details"));
        }
    }
}
