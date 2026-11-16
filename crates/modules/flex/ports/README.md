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

The ports require a tenant and a deadline in `PortContext`. The Flex embedded
provider reads registered stores in the application host. See the
[Flex owner](../README.md#fba-ownership-target) for current composition and storage scope.
