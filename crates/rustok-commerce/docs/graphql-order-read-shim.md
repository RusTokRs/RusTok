# Commerce GraphQL order read compatibility shim

Status: host-runtime-scoped, unvalidated.

The legacy Commerce safe-query source is still included from
`src/graphql/query.rs`, but its `rustok_order` path is now resolved through a
module-local compatibility facade. Complete order detail and filtered-list reads
use `OrderReadPort` with typed requests, explicit locale fallback, a two-second
read deadline, stable owner errors, and the existing response shapes.

Mounted GraphQL execution now scopes the host-selected `CommerceOrderReadRuntime`
from `CommerceGraphqlRuntimeData` into the included safe-query source. The
compatibility facade consumes that scoped runtime. Directly embedded schemas that
do not install the mounted extension retain an explicit in-process fallback from
the resolver-provided database and transactional event bus.

This removes concrete owner-service implementation from complete detail/list
execution while preserving embedded-schema behavior.

The following work remains open and is not promoted by this source change:

- propagate authenticated actor and request channel into the order read context;
- move REST storefront detail/ownership reads in their own atomic change;
- publish wider owner contracts before moving return and order-change reads;
- execute compile, mounted parity, deadline/failure, restart, and remote-adapter evidence.

Return and order-change methods deliberately continue to delegate to the concrete
order owner service. Payment and fulfillment aggregation in admin order detail is
unchanged.

No tests, Cargo commands, formatting, verifiers, workflow checks, or CI results are
claimed by this source checkpoint.
