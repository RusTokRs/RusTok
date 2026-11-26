# rustok-channel-ports

`ChannelIndexIdentityReadPort` returns a tenant's identity generation and UUIDs
from the same Channel snapshot. Read the generation alone, all UUIDs, or the
UUIDs matching a bounded set of canonical slugs. Matches use identity
membership, including inactive Channels.

The optional `grpc-client` and `grpc-server` features provide the
`rustok.channel.v1.ChannelIndexIdentityService` adapter. The standalone service
validates the service credential and tenant allowlist before delegating to the
owner. See [standalone Channel configuration](../README.md#standalone-index-identity-service).

## Public module availability

`ChannelPublicModuleReadPort` checks a tenant-scoped channel by ID or slug and
observes active state and explicit module binding together. Missing ID or a
foreign tenant's ID returns a typed not-found error. An unknown slug, inactive
channel or explicit denial returns false. An active channel without an explicit
module binding allows the module. Calls require a service actor, the public
module-read claim and a bounded deadline.
