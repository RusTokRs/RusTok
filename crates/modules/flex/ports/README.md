# Flex ports

`FlexAttachedSchemaReadPort` returns the ordered active field definitions for a
tenant and attached donor type. `FlexAttachedFieldPolicyReadPort` resolves the
classification and AI-export policy for a bounded set of donor field keys.
The policy result distinguishes explicitly configured policies from the
tenant-private default with export disabled.

`FlexVersionedAttachedSchemaReadPort` returns a complete, ordered snapshot with
a monotonic revision per tenant and donor. Revision zero is the initial state,
including when there are already definitions. An empty snapshot is valid. Every
copy of the same revision carries the same definitions in the same order.

The ports require a tenant and a deadline in `PortContext`. The embedded provider
reads registered stores in the application host. The standalone gRPC adapter
implements the versioned read for Product definitions over the same Flex source.
Enable `grpc-client` or `grpc-server` for the adapter. The server authorizes the
Product schema credential against a tenant allowlist and protects the RPC with
TLS or explicit loopback HTTP in local development. See the
[Flex service setup](../README.md#product-schema-service).
