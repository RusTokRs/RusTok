# Commerce GraphQL order read compatibility shim

Status: complete-and-post-order-reads-cut-over, unvalidated.

The legacy Commerce safe-query source is still included from
`src/graphql/query.rs`, but its `rustok_order` path resolves through a module-local
compatibility facade. Complete order, return, and order-change detail/list reads
use `OrderReadPort` with typed requests, a two-second deadline, stable owner
errors, and the existing response shapes, filters, ordering, and pagination totals.

Mounted GraphQL execution scopes the host-selected `CommerceOrderReadRuntime`
from `CommerceGraphqlRuntimeData` into the included safe-query source. The same
resolver scope derives the actor only from validated `AuthContext` data and carries
the host-resolved `RequestContext.channel_slug` and effective locale into
`PortContext`. Complete-order methods retain their explicit requested/fallback
locale behavior; non-localized post-order methods retain the resolved request
locale as context without adding locale fields to their DTOs.

Directly embedded schemas that do not install the mounted extension retain an
explicit in-process runtime fallback plus service-actor/no-channel context. When
no resolved request locale exists, the non-localized post-order calls use the
truthful `und` marker rather than inventing locale attribution.

The compatibility facade stores only `Arc<dyn OrderReadPort>`. It no longer stores
database/event-bus fields or constructs a concrete order owner service. Typed
`NotFound` failures map back to the existing order, return, or order-change error
variant so GraphQL response behavior remains compatible.

The following work remains open and is not promoted by this source change:

- audit and cut admin return/order-change reads separately without moving mutations;
- execute compile, mounted parity, deadline/failure, restart, and remote-adapter evidence.

Storefront return mutation, storefront refund reads, admin mutations, and admin
payment/fulfillment detail aggregation are unchanged.

No tests, Cargo commands, formatting, verifiers, workflow checks, or CI results are
claimed by this source checkpoint.
