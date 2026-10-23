# Tax Crate API Specification

## Platform Capability Bindings
- **RusToKModule Implementation**: `TaxModule` in `src/module.rs`
- **Module Slug**: `tax`
- **Module Name**: `Tax`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `[]`
- **Permissions**: None (neutral domain calculation and provider seam).
- **Runtime Extensions**: None.
- **Database Migrations**: In-memory calculation domain; exposes empty `MigrationSource` list.

## Domain Collaborations
- **Commerce / Cart**: Invoked by cart, checkout, and order engines to calculate tax lines for items and shipping.
- **Region**: Resolves jurisdiction and country rules via regional baseline policy snapshots.
- **Channel**: Selects tax calculation provider based on tenant/channel overrides before falling back to region providers.

## Minimum Contract Set
- `TaxCalculationPort`: FBA provider port for calculating tax lines on commerce operations (`contracts/tax-fba-registry.json`).
- `TaxService`: High-level coordination service selecting providers by channel/region and delegating calculations.
- `InProcessTaxCalculationPort`: In-process reference calculation adapter supporting baseline rate calculations and customer exemption.

### Input DTOs/Commands
- `TaxCalculationInput`: Scope (`tenant_id`, `channel_id`, `currency`, `customer_id`), taxable amounts (`items`, `shipping`), addresses.
- `TaxableAmount`: Identifier, amount (using `rust_decimal`), category code, and tax inclusion flag.
- `TaxPolicySnapshot`: Region baseline rate, country-specific rules, rounding precision.

### Domain Invariants
- **Non-Negative Amounts**: Taxable amounts must be non-negative; negative amounts fail closed.
- **Decimal Precision**: Float arithmetic is strictly denied (`#![deny(clippy::float_arithmetic)]`); tax amounts round half away from zero at currency precision.
- **Currency Parity**: Line tax currencies must strictly match the calculation input currency.
- **Tax Exempt Handling**: Tax-exempt customers produce zero-rate empty tax lines deterministically.
- **Provider Resolution Precedence**: Explicit channel provider override takes precedence over region baseline provider.

### Events / Outbox Side Effects
- Neutral calculation domain; does not emit durable outbox events directly.

### Errors / Failure Codes
- `TaxError::Validation`: Missing required address or invalid negative amounts.
- `TaxError::UnknownProvider`: Requested calculation provider is not registered in the system.
- `TaxError::CalculationFailed`: Provider failure or currency mismatch in calculated lines.
