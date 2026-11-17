# rustok-channel-ports

`ChannelIndexIdentityReadPort` returns a tenant's identity generation and UUIDs
from the same Channel snapshot. Read the generation alone, all UUIDs, or the
UUIDs matching a bounded set of canonical slugs. Matches use identity
membership, including inactive Channels.

The optional `grpc-client` and `grpc-server` features provide the
`rustok.channel.v1.ChannelIndexIdentityService` adapter. The standalone service
validates the service credential and tenant allowlist before delegating to the
owner. See [standalone Channel configuration](../README.md#standalone-index-identity-service).
