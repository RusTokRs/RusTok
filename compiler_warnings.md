# Список предупреждений компилятора (Compiler Warnings)

Проект компилируется успешно (`cargo check` завершается без ошибок), но присутствуют предупреждения компилятора (unused imports, unused variables, unused methods). Ниже приведён их детальный список, сгруппированный по крейтам и файлам.

---

## 1. Крейт `rustok-commerce` (`crates/rustok-commerce`)

### [cart.rs](file:///d:/RusTok/crates/rustok-commerce/src/graphql/mutations/cart.rs)
- **Строка 1:** Неиспользуемый импорт `FieldError`:
  ```rust
  use async_graphql::{Context, FieldError, Object, Result};
  ```
- **Строка 2:** Неиспользуемый импорт `rust_decimal::Decimal`.
- **Строка 4:** Неиспользуемый импорт `GraphQLError`.
- **Строка 7:** Неиспользуемые импорты `Permission` и `locale_tags_match`.
- **Строка 8:** Неиспользуемый импорт `rustok_inventory::check_variant_availability_for_public_channel`.
- **Строка 9:** Неиспользуемый импорт `rustok_pricing::PriceResolutionContext`.
- **Строка 10:** Неиспользуемые импорты `ColumnTrait`, `EntityTrait` и `QueryFilter`.
- **Строка 11:** Неиспользуемый импорт `serde_json::Value`.
- **Строка 12:** Неиспользуемый импорт `std::str::FromStr`.
- **Строки 16-26:** Неиспользуемые импорты бизнес-логики и сервисов (например, `CatalogService`, `CheckoutService` и др.).
- **Строка 30:** Неиспользуемый импорт `require_commerce_permission`.

### [catalog.rs](file:///d:/RusTok/crates/rustok-commerce/src/graphql/mutations/catalog.rs)
- **Строка 1:** Неиспользуемый импорт `FieldError`.
- **Строка 2:** Неиспользуемый импорт `Decimal`.
- **Строки 4-5:** Неиспользуемые импорты `AuthContext`, `GraphQLError`, `RequestContext`, `TenantContext`.
- **Строка 7:** Неиспользуемый импорт `locale_tags_match`.
- **Строка 8:** Неиспользуемый импорт `check_variant_availability_for_public_channel`.
- **Строка 9:** Неиспользуемый импорт `PriceResolutionContext`.
- **Строка 10:** Неиспользуемые импорты `ColumnTrait`, `EntityTrait`, `QueryFilter`.
- **Строка 11:** Неиспользуемый импорт `Value`.
- **Строка 12:** Неиспользуемый импорт `FromStr`.
- **Строки 16-26:** Неиспользуемые импорты бизнес-сервисов (`CartService`, `CheckoutService` и др.).

### [checkout.rs](file:///d:/RusTok/crates/rustok-commerce/src/graphql/mutations/checkout.rs)
- **Строка 1:** Неиспользуемый импорт `FieldError`.
- **Строка 2:** Неиспользуемый импорт `Decimal`.
- **Строка 4:** Неиспользуемый импорт `GraphQLError`.
- **Строка 7:** Неиспользуемый импорт `locale_tags_match`.
- **Строка 8:** Неиспользуемый импорт `check_variant_availability_for_public_channel`.
- **Строка 9:** Неиспользуемый импорт `PriceResolutionContext`.
- **Строка 10:** Неиспользуемые импорты `ColumnTrait`, `EntityTrait`, `QueryFilter`.
- **Строка 11:** Неиспользуемый импорт `Value`.
- **Строка 12:** Неиспользуемый импорт `FromStr`.
- **Строки 16-26:** Неиспользуемые импорты сервисов и утилит (`CatalogService`, `CreateReturnDecisionInput` и др.).

### [fulfillment.rs](file:///d:/RusTok/crates/rustok-commerce/src/graphql/mutations/fulfillment.rs)
- **Строки 4-5:** Неиспользуемые импорты `GraphQLError` и `RequestContext`.
- **Строка 7:** Неиспользуемый импорт `locale_tags_match`.
- **Строка 8:** Неиспользуемый импорт `check_variant_availability_for_public_channel`.
- **Строка 9:** Неиспользуемый импорт `PriceResolutionContext`.
- **Строка 10:** Неиспользуемые импорты `ColumnTrait`, `EntityTrait`, `QueryFilter`.
- **Строка 11:** Неиспользуемый импорт `Value`.
- **Строки 16-26:** Неиспользуемые импорты сервисов (`CartService`, `CatalogService` и др.).

### [helpers.rs](file:///d:/RusTok/crates/rustok-commerce/src/graphql/mutations/helpers.rs)
- **Строка 3:** Неиспользуемый импорт `TenantContext`.
- **Строка 4:** Неиспользуемый импорт `Permission`.
- **Строки 19-23:** Неиспользуемые импорты `CatalogService`, `ExchangeDifferenceRefundInput` и др.
- **Строка 26:** Неиспользуемые импорты `MODULE_SLUG` и `require_commerce_permission`.

### [pricing.rs](file:///d:/RusTok/crates/rustok-commerce/src/graphql/mutations/pricing.rs)
- **Строка 1:** Неиспользуемый импорт `FieldError`.
- **Строка 2:** Неиспользуемый импорт `Decimal`.
- **Строки 4-5:** Неиспользуемые импорты `AuthContext`, `GraphQLError`, `RequestContext`.
- **Строка 7:** Неиспользуемый импорт `locale_tags_match`.
- **Строка 8:** Неиспользуемый импорт `check_variant_availability_for_public_channel`.
- **Строка 9:** Неиспользуемый импорт `PriceResolutionContext`.
- **Строка 10:** Неиспользуемые импорты `ColumnTrait`, `EntityTrait`, `QueryFilter`.
- **Строка 11:** Неиспользуемый импорт `Value`.
- **Строка 12:** Неиспользуемый импорт `FromStr`.
- **Строки 16-26:** Неиспользуемые импорты сервисов (`CatalogService`, `CheckoutService` и др.).

---

## 2. Крейт `rustok-auth-admin` (`crates/rustok-auth/admin`)

### [profile.rs](file:///d:/RusTok/crates/rustok-auth/admin/src/ui/profile.rs)
- **Строка 10:** Неиспользуемый импорт `ApiError`:
  ```rust
  use crate::transport::{update_profile, ApiError};
  ```

### [users.rs](file:///d:/RusTok/crates/rustok-auth/admin/src/ui/users.rs)
- **Строка 1:** Неиспользуемые импорты `Engine` и `STANDARD` из `base64`.

### [oauth_apps.rs](file:///d:/RusTok/crates/rustok-auth/admin/src/ui/oauth_apps.rs)
- **Строка 5:** Неиспользуемый импорт `uuid::Uuid`.

---

## 3. Крейт `rustok-pages-admin` (`crates/rustok-pages/admin`)

### [leptos.rs](file:///d:/RusTok/crates/rustok-pages/admin/src/ui/leptos.rs)
- **Строка 1119:** Неиспользуемая переменная `page_id`:
  ```rust
  let page_id = page.id.clone();
  ```

---

## 4. Крейт `rustok-pricing-admin` (`crates/rustok-pricing/admin`)

### [mod.rs](file:///d:/RusTok/crates/rustok-pricing/admin/src/core/mod.rs)
- **Строка 9:** Неиспользуемый импорт `format_price_scope`.

---

## 5. Крейт `rustok-admin` (`apps/admin`)

### [governance_form.rs](file:///d:/RusTok/apps/admin/src/features/modules/components/detail/governance_form.rs)
- **Строка 13:** Неиспользуемый импорт `status_eq`.
- **Строка 33:** Неиспользуемая переменная `set_governance_intent_action` (тип `WriteSignal<Option<String>>`).

### [module_detail_panel.rs](file:///d:/RusTok/apps/admin/src/features/modules/components/module_detail_panel.rs)
- **Строка 22:** Неиспользуемый импорт `Input`.
- **Строки 30-49:** Неиспользуемые импорты утилит иBadge-классов (такие как `approval_override_warning_lines`, `metadata_status_badge_classes` и др.).

### [module_composition_graphql_guard.rs](file:///d:/RusTok/apps/admin/tests/module_composition_graphql_guard.rs)
- **Строки 37, 55, 74, 92, 113, 152, 194, 231, 261, 312, 346, 377, 410, 444, 476, 508, 537, 598, 667, 686, 745, 776, 933, 974, 1158, 1482:**
  Неиспользуемая переменная `crate_root`:
  ```rust
  let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
  ```

---

## 6. Крейт `rustok-server` (`apps/server`)

### [types.rs](file:///d:/RusTok/apps/server/src/modules/manifest/types.rs)
- **Строка 2:** Неиспользуемые импорты `Version` и `VersionReq` из `semver`.
- **Строка 5:** Неиспользуемый импорт `std::path::PathBuf`.

### [plans.rs](file:///d:/RusTok/apps/server/src/modules/manifest/plans.rs)
- **Строка 2:** Неиспользуемый импорт `BuildModuleSpec`.
- **Строка 3:** Неиспользуемые импорты `Path` и `PathBuf` из `std::path`.

### [validation.rs](file:///d:/RusTok/apps/server/src/modules/manifest/validation.rs)
- **Строка 2:** Неиспользуемый импорт `BuildModuleSpec`.
- **Строка 3:** Неиспользуемый импорт `ModuleRegistry` из `rustok_core`.

### [mod.rs](file:///d:/RusTok/apps/server/src/services/registry_governance/mod.rs)
- **Строка 427:** Неиспользуемые импорты `rejected_publish_request_can_retry` и `validate_registry_artifact_bundle`.

### [tenant.rs](file:///d:/RusTok/apps/server/src/middleware/tenant.rs)
- **Строка 446:** Неиспользуемый (dead_code) метод `set_cached_tenant`:
  ```rust
  async fn set_cached_tenant(...)
  ```

### [tests.rs](file:///d:/RusTok/apps/server/src/modules/manifest/tests.rs)
- **Строка 2:** Неиспользуемый импорт `serial` из `serial_test`.
- **Строка 3:** Неиспользуемый импорт `HashMap` из `std::collections`.
- **Строка 4:** Неиспользуемый импорт `tempdir` из `tempfile`.
