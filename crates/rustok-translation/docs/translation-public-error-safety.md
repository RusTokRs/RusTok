# Translation shared public-error safety

Status: **source-ready / unvalidated**

## Scope

This slice closes the public and diagnostic payload gap in Translation-owned `map_translation_public_error`, the shared mapper used by GraphQL and native admin consumers.

The public structs, error kinds, stable codes, retryability, generated correlation id, display reference, shared export, and consumer composition remain unchanged.

## Public envelopes

The mapper keeps four public kinds:

- forbidden;
- not found;
- bad input;
- internal.

Forbidden, retryable-internal, and ordinary-internal messages are unchanged. Not-found errors now use `Translation resource was not found`. Bad-input errors now use `Translation request is invalid`.

The previous `error.to_string()` messages could contain owner workflow state, locale, revision, reason, provider, or validation payload. Those details are no longer returned through GraphQL or native server-function errors.

## Private diagnostics

The structured event retains only:

- a closed static error class;
- owner operation;
- boundary;
- public code;
- retryability;
- correlation id.

The complete `TranslationError`, provider code/message, database error, workflow state, locale, revision, cancellation/recovery reason, and validation payload are not recorded.

## Preserved behavior

- GraphQL still maps the four public kinds to permission-denied, not-found, bad-user-input, and internal GraphQL errors;
- native admin still renders the same `TranslationPublicError` through `ServerFnError`;
- all existing error variants remain in their previous kind/code/retryability groups;
- `Uuid::new_v4()` still generates one correlation id per mapped failure;
- `Display` still renders message, code, and reference;
- dispatch, permissions, requests, responses, and transport selection are unchanged.

## Evidence

- `crates/rustok-translation/contracts/evidence/translation-public-error-safety-source.json`
- `crates/rustok-translation/contracts/evidence/translation-public-error-safety-source-review.json`
- `scripts/verify/verify-translation-public-error-safety.mjs`

## Remaining gaps

Compile, focused-verifier execution, GraphQL runtime evidence, and native runtime evidence remain open. The broader ecommerce mapper cleanup remains open.

No test, verifier, formatter, Cargo, workflow, or CI command was executed for this source slice.
