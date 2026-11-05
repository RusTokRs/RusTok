# Region Crate API Specification

## Public Modules
- `dto`: Input and output DTOs (`CreateRegionInput`, `UpdateRegionInput`, `RegionResponse`).
- `error`: Strongly typed domain error variants (`RegionError`, `RegionResult`).
- `migrations`: SeaORM migration chain for region persistence.
- `module`: Lifecycle descriptor `RegionModule` implementing `RusToKModule`.
- `ports`: Owner-defined cross-boundary read port (`RegionReadPort`, `RegionReadProjection`).
- `services`: Domain services for region management and translation lifecycle.
- `translation_changes`: Translation journal change records and lifecycle.
- `translation_target`: Registration and provider for the translation engine.

## Platform Capability Bindings
- **RusToKModule Implementation**: `RegionModule` in `src/module.rs`
- **Module Slug**: `region`
- **Module Name**: `Region`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `[]`
- **Permissions**:
  - `REGIONS_CREATE`
  - `REGIONS_READ`
  - `REGIONS_UPDATE`
  - `REGIONS_DELETE`
  - `REGIONS_LIST`
  - `REGIONS_MANAGE`
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` (6 migrations including country tax policies and translation change journal).

## Domain Collaborations
- **Translation Targets**: Implements `RegionTranslationTargetProvider` for localized region presentation and translation journal synchronization.
- **Cart**: Native tax recalculation calls `RegionService::read_tax_policy_in_tx` with the active Cart transaction and receives `RegionTaxPolicy`.
- **Outbox**: Translation target application uses owner operation receipts. Region commands append the Region translation change journal in their transaction.
- **FBA Port**: Exposes `RegionReadPort` for decoupled cross-boundary reading of regional configurations, currencies, and countries.

## Minimum Contract Set
- `RegionReadPort`: Decoupled read port for resolving regions, tax rates, and country associations.
- `RegionService`: Create, update, fetch, list, and resolve-by-country operations on regional aggregates.
- `RegionService::read_tax_policy_in_tx(&DatabaseTransaction, tenant_id, region_id, channel_id) -> Result<Option<RegionTaxPolicy>, DbErr>` returns the tenant-scoped base rate, country rules, baseline provider, and selected channel provider from one database statement. Missing and foreign regions return `None`. Storage failures propagate to the native transaction caller.
- `RegionTranslationService`: Regional translation snapshot and exact locale applying.

### Input DTOs/Commands
- `CreateRegionInput`: Explicit translations, currency, baseline tax provider/rate/inclusion, countries, optional country policies, and metadata.
- `UpdateRegionInput`: Optional updates to those owner fields.
- `RegionCountryTaxPolicyInput`: Country code, rate, and inclusion rule.
- `RegionTaxPolicy`: Native tax policy projection without persistence entities or localization data. Region resolves `channel_tax_provider_ids` into `channel_provider_id` using the selected Cart channel.

### Domain Invariants
- **Non-Empty Name**: Region name must not be blank.
- **Currency Code Shape**: Currency code must contain three ASCII letters and is normalized to uppercase. This does not validate ISO membership.
- **Country Uniqueness**: The shipped integrity migration rejects duplicate country ownership within a tenant.
- **Tax Rate Precision**: Tax rates must be valid decimal percentages.

### Events / Outbox Side Effects
- Region create and localized update append a translation change in the same transaction. The translation target uses the owner receipt contract for apply operations.

### Errors / Failure Codes
- `RegionError::RegionNotFound`: The requested region aggregate does not exist.
- `RegionError::Validation`: Input validation failed.
- `RegionError::Database`: Database storage failure.
