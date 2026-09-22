# Channel resolution pipeline and typed policy trajectory

- Date: 2026-03-27
- Status: Accepted

## Context

`rustok-channel` started as a v0 baseline with a simple runtime order `header -> query -> host -> default`.

This is sufficient for the pilot stage, but not enough as the final platform architecture:

- resolution logic lived in server middleware, not in a domain module;
- the explicit default channel already appeared, but the next step should not turn into ad-hoc `tenant-level default rules`;
- as the platform grows further, richer resolution predicates (`host`, `oauth_app`, `surface`, `locale`) will be needed, but introducing a scripting/generic rule engine prematurely will create debt and blur invariants.

The final trajectory needs to be fixed now, while the codebase still allows an architectural shift without an expensive break.

## Decision

The following final channel resolution model is adopted:

1. explicit selectors;
2. built-in typed target-resolution slices;
3. tenant-scoped typed resolution policies;
4. explicit default channel;
5. unresolved request.

Key decisions:

- `tenant-level default rules` are not introduced as a separate architectural concept;
- there is only one terminal fallback: the tenant's explicit default channel;
- runtime resolution is moved from server middleware to the `rustok-channel` domain layer;
- the shared runtime contract is defined as a typed pipeline:
  - `RequestFacts`
  - `ResolutionDecision`
  - `ResolutionTraceStep`
- the current host-based lookup by `web_domain` is treated as a built-in typed resolution slice, not as the foundation for a generic rule engine;
- future configuration for richer matching will be called `tenant-scoped typed resolution policies` and will be placed before the explicit default channel;
- the policy layer must not be Turing-complete:
  only typed predicates/action model, without scripting and without arbitrary eval.

`rustok-api` remains the owner of the host-facing `ChannelResolutionSource`, while the domain resolver in `rustok-channel` holds its own resolution contract and maps to the shared host contract at the server boundary.

## Consequences

Positives:

- precedence order now becomes a domain invariant, not a middleware detail;
- a typed seam appears for future policy sets without immediately introducing a rule engine;
- debug/observability can be built on `ResolutionTraceStep` rather than on implicit middleware branches;
- the explicit default channel remains a deterministic terminal fallback.

Negatives and follow-up:

- an additional resolver layer and temporary mapping between domain origin and host-facing source contract appears in the code;
- the next stage is no longer about "yet another fallback", but about storage/model/admin/runtime rollout for typed policies;
- the current built-in host slice will later need to either be integrated into the common policy engine, or explicitly kept as a fast-path policy family without duplicating semantics.
