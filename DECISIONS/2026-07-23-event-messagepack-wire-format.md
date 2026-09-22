# ADR: Event MessagePack wire format

- Status: accepted
- Date: 2026-07-23

## Context

`rustok-events` exposes internally tagged Serde event enums so JSON payloads
are explicit and stable. Postcard cannot deserialize that Serde representation;
its decoder rejects the `deserialize_any` operation used by internally tagged
enums. A configured Postcard transport could publish bytes but could not
reliably read either root or typed-contract envelopes back.

## Decision

The Iggy transport supports JSON and MessagePack. MessagePack uses the stable
`rmp-serde` library and the configuration value `messagepack`; the Postcard
format and public `PostcardSerializer` are removed without a compatibility
alias.

Both formats preserve the same validated envelope semantics. JSON timestamps
are RFC 3339 strings and MessagePack timestamps are UTC microseconds; both
decode into `DateTime<Utc>`. Every decoded root envelope is revalidated before
it reaches a consumer.

## Consequences

- Binary transport round-trips root and typed-contract envelopes, including
  the tagged event families used by the published contracts.
- Deployments must change any `postcard` serialization configuration to
  `messagepack` before using the new transport version.
- JSON remains the default and human-readable interoperability profile.
