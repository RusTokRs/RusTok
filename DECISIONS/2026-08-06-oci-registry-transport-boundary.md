# ADR: Platform-Owned OCI Registry Transport Boundary

## Status

Accepted.

## Context

Module artifact admission and publication require an OCI Distribution client
that can enforce HTTPS, certificate verification, redirect refusal, proxy
isolation, request deadlines, retry bounds, response-size bounds, content
encoding rules, and credential-origin restrictions. The previously used
upstream client owns its internal HTTP client and does not expose those
controls. Delegating them to ambient process settings or an optional egress
deployment leaves an unverifiable fail-open path.

## Decision

`rustok-modules` owns the small OCI Distribution HTTP subset required by the
current control plane: digest/tag manifest reads, streaming blob reads,
monolithic blob uploads, and manifest uploads. It constructs one strict
`reqwest` client with HTTPS-only routing, verified TLS defaults, no redirects,
no process/system proxy, explicit connection/request deadlines, all automatic
decompression disabled, and the HTTP library retry mechanism disabled.

The transport applies the typed `OciRegistryTransportPolicy` itself. It
performs only the policy-bounded retry loop, holds a semaphore for the complete
response stream, rejects encoded responses, limits both transfer and
decompressed response size, and accepts upload locations only on the original
HTTPS origin. Bearer authentication may obtain an anonymous token from a
different HTTPS token host, but Basic credentials are sent only to the original
registry origin and are never forwarded to a token service or upload location
on another host.

`oci-distribution` remains a data-model and `RegistryAuth` dependency. Its
network client is not constructed anywhere in repository-owned production
code.

## Consequences

The control plane has one auditable egress implementation for module OCI
traffic, independent of environment proxy variables and upstream HTTP defaults.
The platform intentionally implements only the OCI API operations it needs;
unsupported OCI workflows fail closed instead of inheriting a broad client
surface. Registry deployments can still supply transparent network routing,
but that routing cannot alter the client's redirect, proxy, timeout, retry,
credential-origin, or size policies.
