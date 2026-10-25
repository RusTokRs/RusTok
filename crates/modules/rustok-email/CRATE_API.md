# rustok-email / CRATE_API

## Public Modules
`config`, `error`, `module`, `ports`, `service`, `template`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `EmailModule` implementing `RusToKModule` (slug `"email"`, kind `ModuleKind::Core`) and `MigrationSource`.
- **Email Service & Transport**:
  - `EmailService` (`Disabled`, `Smtp`) providing transactional and template-based email delivery.
  - `EmailConfig` and `SmtpConfig` parsed from environment or configuration profiles.
  - `SmtpEmailSender` and `TransactionalEmailSender` traits.
- **Template Rendering**:
  - `EmailTemplateProvider` leveraging Tera for localized HTML and plain text email rendering.
  - `RenderedEmail` containing subject, HTML body, and optional plain text content.
- **Ports & Transports**:
  - `EmailDeliveryPort` (`send_transactional_email`)
  - `require_email_delivery_policy` enforcing write semantics, deadlines, and idempotency keys.
  - `EmailDeliveryRequest`, `EmailDeliveryReceipt`, `EmailProviderMode`.

## Domain Collaborations
- **Core Platform**: Integrates with `rustok-core` module system and `rustok-api` port context, call policies, and error types.
- **Workflow & Auth**: Consumes transactional delivery requests for authentication password resets, account verification, and notification delivery.

## Primary Public Types and Signatures
- `pub struct EmailModule`
- `pub enum EmailService`
- `pub trait EmailDeliveryPort`
- `pub struct EmailDeliveryRequest`
- `pub struct EmailDeliveryReceipt`
- `pub enum EmailProviderMode`
- `pub struct EmailConfig`
- `pub struct SmtpConfig`
- `pub struct EmailTemplateProvider`
- `pub struct RenderedEmail`
- `pub enum EmailError`

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`

## Common AI Mistakes
- Using wildcard re-exports instead of explicit named imports.
- Calling raw SMTP transport functions directly without going through `EmailDeliveryPort` or `EmailService`.
- Omitting deadline or idempotency key requirements on delivery port contexts.
- Ignoring recipient validation and allowing control characters (`\r`, `\n`) that enable header injection.

## Minimum Contract Set

### Input DTOs/Commands
- `EmailDeliveryRequest` (`template_id`, `locale`, `to`, `vars`)
- `EmailConfig` (`smtp`, `from_address`, `from_name`)
- `SmtpConfig` (`host`, `port`, `username`, `password`, `starttls`, `ssl`)

### Domain Invariants
- Recipient validation: Recipient addresses must be valid RFC-compliant emails without carriage returns or line feeds to prevent CRLF injection.
- Required deadline & idempotency: Transactional email delivery requires write policy semantics with non-empty idempotency key and explicit deadline.
- Safe disabled fallback: When email transport is disabled (`EmailService::Disabled`), delivery port calls succeed gracefully returning `EmailProviderMode::DisabledNoop` without panicking.
- Template resolution: Template rendering validates that both HTML/text components and requested localization exist before attempting transmission.

### Events / Outbox Side Effects
- Successful or failed transactional deliveries emit structured telemetry log events; future outbox integration records delivery outcomes.

### Errors / Failure Codes
- `EmailError` variants: `Disabled`, `Template`, `InvalidAddress`, `Build`, `SmtpConfig`, `Send`.
- `PortError` mapped uniformly to `PortErrorKind::Validation`, `PortErrorKind::Timeout`, `PortErrorKind::InvariantViolation`, or `PortErrorKind::Unavailable`.
