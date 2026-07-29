# Commerce GraphQL order read compatibility shim

Status: request-context-scoped, unvalidated.

The legacy Commerce safe-query source is still included from
`src/graphql/query.rs`, but its `rustok_order` path is now resolved through a
module-local compatibility facade. Complete order detail and filtered-list reads
use `OrderReadPort` with typed requests, explicit locale fallback, a two-second
read deadline, stable owner errors, and the existing response shapes.

Mounted GraphQL execution scopes the host-selected `CommerceOrderReadRuntime`
from `CommerceGraphqlRuntimeData` into the included safe-query source. The same
resolver scope derives the order actor only from validated `AuthContext` request
data and carries the host-resolved `RequestContext.channel_slug` into
`PortContext`. Unauthenticated mounted reads use a stable service actor.

Directly embedded schemas that do not install the mounted extension retain an
explicit in-process runtime fallback and a service-actor/no-channel context
fallback. This preserves embedded-schema behavior without inventing user or
channel attribution.

The following work remains open and is not promoted by this source change:

- move REST storefront detail/ownership reads in their own atomic change;
- publish wider owner contracts before moving return and order-change reads;
- execute compile, mounted parity, deadline/failure, restart, and remote-adapter evidence.

Return and order-change methods deliberately continue to delegate to the concrete
order owner service. Payment and fulfillment aggregation in admin order detail is
unchanged.

No tests, Cargo commands, formatting, verifiers, workflow checks, or CI results are
claimed by this source checkpoint.
