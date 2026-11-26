# rustok-blog-ports

Published contracts for the [Blog owner](../README.md). Public article reads use
the canonical `PortContext`. A service supplies its admitted tenant, channel and
locale, the Blog public-read claim, and a bounded deadline.

`BlogPublishedPostReadPort` resolves an admitted published article by slug or
historical route. Hidden, unpublished and missing posts have the same absence
result. The projected article includes its publication timestamp and locale
provenance. The default package contains only these contracts.

See [CRATE_API.md](CRATE_API.md) for the contract and the
[Blog implementation plan](../docs/implementation-plan.md#first-standalone-blog-capability)
for the optional standalone profile.
