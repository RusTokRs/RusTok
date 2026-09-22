# ADR: User-registration event PII boundary

- Status: accepted
- Date: 2026-07-23

## Context

The established root `user.registered` event included an email address. Event
payloads are persisted in the outbox, may be relayed to external brokers, and
can be replayed or sent to multiple consumer owners. Email is contact data and
is not needed to establish that a user account exists.

Repository review found only contract tests constructing the event; no
production publisher or consumer used the email-carrying payload. Keeping a
legacy event only to preserve an unused initial implementation would continue
to expose contact data in a durable shared contract.

## Decision

Remove `DomainEvent::UserRegistered` and its `user.registered` schema
atomically. Introduce `DomainEvent::UserAccountRegistered` with event type
`user.account_registered`, schema version `1`, and exactly one payload field:
`user_id`.

Email addresses and every other contact attribute remain private to the auth or
user owner. A consumer needing such data must use an authorized owner read
contract; it must not recover contact data from an event payload. No compatibility
alias or dual event publication is retained because there is no production
publisher or reader to migrate.

## Consequences

- Durable event records no longer carry email for user-registration facts.
- The event schema release artifact and tests make future contact-data additions
  visible for review.
- A future external user-registration integration needs its own minimized,
  consent-aware contract instead of extending the internal root event casually.
