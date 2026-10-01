---
id: doc://docs/guides/multilingual-demo.md
kind: guide
language: markdown
status: verified
owner: platform-architecture
---
# Multilingual Architecture and Demo Data Guide

This document is the canonical reference for developers and AI agents on **multilingualism (i18n / l10n)** across RusToK and how **multilingual starter / demo blueprints** operate.

---

## 1. Core Platform Architecture: Non-Disableable Translation Module

In RusToK, multilingual capability is supported through `rustok-translation`:
- **`rustok-translation` Module**: Declared in [`modules.toml`](../../modules.toml) and [`modules.local.toml`](../../modules.local.toml).
- **Policy and Lifecycle**: The kernel policy engine ([`crates/modules/rustok-modules/src/policy.rs`](../../crates/modules/rustok-modules/src/policy.rs)) and server lifecycle service ([`apps/server/src/services/module_lifecycle.rs`](../../apps/server/src/services/module_lifecycle.rs)) enforce module invariants and lifecycle state.
- **System Boundaries**:
  - `rustok-translation` owns Translation Memory (TM), terminology glossaries, machine translation orchestration (`MachineTranslationPort`), translation review workflows (jobs, proposals, apply CAS), and exchange packages (XLIFF / JSON interchange).
  - `rustok-translation-targets` provides the dependency-neutral target provider contracts implemented by domain modules (`rustok-pages`, `rustok-taxonomy`, `rustok-product`).
  - `rustok-ui-i18n` provides framework-agnostic ICU4X-normalized Fluent message catalogs (`.ftl`) for Admin and Storefront UI surfaces.

---

## 2. Platform Localization Invariants

### 2.1 The Canonical Locale Matching Rule
Read-side localized content resolution across Storefront and Admin strictly adheres to the platform fallback chain:

$$\text{Requested Locale} \longrightarrow \text{Tenant Default Locale} \longrightarrow \text{First Available Translation}$$

- **Normalization**: All locale tags are normalized via ICU4X in [`rustok-ui-i18n`](../../crates/ui/rustok-ui-i18n/README.md) (e.g. `ru-RU` $\rightarrow$ `ru`, `en-US` $\rightarrow$ `en`).
- **No silent empty strings**: If a requested language translation is missing, the resolver falls back to the tenant's primary default language rather than rendering a blank field.

### 2.2 Content Entity Translations (`*_translations`)
Domain content does not store hardcoded single-language text in entity root tables. Instead, localized fields reside in dedicated translation tables with tenant-composite foreign keys:
- `page_translations` (`page_id`, `locale`, `title`, `slug`, `meta_title`, `meta_description`)
- `page_bodies` (`page_id`, `locale`, `document`)
- `blog_category_translations` (`category_id`, `locale`, `name`, `description`)
- `forum_category_translations` (`category_id`, `locale`, `name`, `description`)
- `menu_translations` (`menu_id`, `locale`, `name`)
- `menu_item_translations` (`item_id`, `locale`, `title`)
- `brand_translations` (`brand_id`, `locale`, `name`, `description`)
- `taxonomy_term_translations` (`term_id`, `locale`, `name`, `description`)

---

## 3. Multilingual Starter Blueprints & Demo Data

Demo data is designed to showcase the platform's native multilingual abilities out of the box.

### 3.1 Blueprint Locale Configuration
Every `StarterBlueprint` specifies its target `locale`:
```json
{
  "schema_version": "1.0",
  "id": "default-starter-ru",
  "name": "RusToK Russian Starter Pack",
  "locale": "ru",
  "content": { ... }
}
```

The import engine ([`crates/utils/rustok-starter/src/importer.rs`](../../crates/utils/rustok-starter/src/importer.rs)) passes `blueprint.locale` to all domain drivers:
1. **Pages Driver** ([`pages.rs`](../../crates/utils/rustok-starter/src/drivers/pages.rs)):
   Creates `PageTranslationInput` and `PageBodyInput` matching `blueprint.locale`. The page is published via `PageBuilderReviewedPublishRuntime`, rendering the landing page for that language on `/`.
2. **Blog Driver** ([`blog.rs`](../../crates/utils/rustok-starter/src/drivers/blog.rs)):
   Creates categories and blog posts with the specified locale tag, ensuring blog articles appear in the selected language catalog.
3. **Forum Driver** ([`forum.rs`](../../crates/utils/rustok-starter/src/drivers/forum.rs)):
   Creates categories and starter discussion topics localized to `blueprint.locale`.
4. **Navigation Driver** ([`navigation.rs`](../../crates/utils/rustok-starter/src/drivers/navigation.rs)):
   Creates `MenuItemTranslationInput` and `MenuTranslationInput` with `blueprint.locale` and binds the menu to the channel slot.

### 3.2 Dual-Locale & Multi-Blueprint Tenant Seeding
A tenant can host multiple language packs simultaneously.
When seeding demo data for a multilingual store:
1. **Primary Language Import (e.g. Russian `ru`)**:
   ```bash
   rustok-cli starter import --tenant-slug demo --starter default
   ```
2. **Secondary Language Import (e.g. English `en`)**:
   ```bash
   rustok-cli starter import --tenant-slug demo --file ./crates/utils/rustok-starter/blueprints/default_en.json
   ```

Because all drivers follow idempotent skip-or-add semantics:
- When a page or navigation menu already exists with the same identifier, the driver registers the additional translation for the new locale without duplicate parent records.
- Visitors switching languages on the Storefront (`/ru` $\leftrightarrow$ `/en`) immediately see the respective localized page bodies, menu titles, and blog posts.

---

## 4. Multilingual Demo Workflows with `rustok-translation`

The non-disableable `rustok-translation` module enables advanced demo scenarios:

### 4.1 Automated Machine Translation of Demo Content
Once starter content is imported in the primary language (`ru`), developers can demonstrate AI translation capabilities:
1. Open Admin Panel $\rightarrow$ **Translation** (`/admin/translation`).
2. Run inventory scan: `rustok-translation` discovers untranslated items in `pages`, `blog`, and `taxonomy`.
3. Create a Translation Job to generate target locale proposals (e.g. `ru` $\rightarrow$ `en`).
4. `rustok-translation` generates translation proposals via `MachineTranslationPort` (mock provider or `rustok-ai` LLM).
5. Apply proposals: the reviewed translations are atomically written back to the target domain module tables (`page_translations`, `blog_post_translations`).

### 4.2 Translation Memory (TM) & Glossary Demo
The default demo includes pre-seeded glossary entries and translation memory pairs:
- **Glossary**: E-commerce terminology (e.g. `Корзина` $\leftrightarrow$ `Shopping Cart`, `Оформить заказ` $\leftrightarrow$ `Checkout`).
- **Translation Memory**: Standard UI strings and recurring marketing taglines.
- **Reviewer Queue**: Demonstrates the workflow from Draft $\rightarrow$ In Review $\rightarrow$ Approved $\rightarrow$ Applied.

---

## 5. Agent Instructions: Working with Multilingual Demo

When modifying demo blueprints or developing localization features:

1. **Never hardcode single-locale strings**:
   - Always ensure DTOs accept and propagate `locale`.
   - Localized database writes must target `*_translations` tables or domain services accepting translation inputs.
2. **Respect `rustok-translation` as a Core Module**:
   - Do NOT attempt to make `rustok-translation` optional or add conditional disabling logic.
   - Any module contributing translatable resources must implement `TranslationTargetProvider` from `rustok-translation-targets` rather than inventing an ad-hoc translation mechanism.
3. **Verification Checklist**:
   - Run translation crate checks:
     ```bash
     cargo check -p rustok-translation
     cargo clippy -p rustok-translation -- -D warnings
     ```
   - Run starter tests:
     ```bash
     cargo check -p rustok-starter --tests
     ```
   - Verify architecture boundaries:
     ```bash
     python scripts/architecture_dependency_guard.py
     ```
