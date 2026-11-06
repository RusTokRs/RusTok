# rustok-graphql

For full public interface and contract definitions, see [CRATE_API.md](CRATE_API.md).

`rustok-graphql` owns the framework-agnostic GraphQL HTTP client contracts for
RusToK UI adapters.

It provides request/response types, one shared pooled HTTP client, the shared
HTTP execution function, error mapping and persisted-query extension helpers.
It does not depend on Leptos, Dioxus, Next.js, GraphQL schema crates or host UI
context.

The client does not automatically retry GraphQL operations. Retry safety belongs
to the operation owner because an HTTP `POST` does not distinguish an idempotent
query from a mutation. Browser and native targets use the same no-hidden-retry
contract. Native requests use the shared client timeout; browser request timing
is owned by the browser fetch runtime and higher-level cancellation policy.

## Browser execution

Browser execution keeps the whole fetch/response lifecycle in an origin-thread
checked `send_wrapper`, already used by the Leptos browser runtime. Owned Send
DTOs can therefore be loaded by UI resources without a package-local transport
wrapper. Browser futures must be polled and dropped on their originating JS
thread; moving active browser IO between workers is not supported. Native
execution keeps its ordinary thread-safe client and error behavior.

## Known Limitations / Pending Implementation

The browser thread restriction is intentional. Mounted network/error/cancellation
execution remains pending in the owning UI clients; native unit tests and WASM
compilation do not establish that evidence.
