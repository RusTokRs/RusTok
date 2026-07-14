---
id: doc://docs/operations/env-rustok-public-url.md
kind: operations_documentation
language: en
source_language: en
entities:
  - env://RUSTOK_PUBLIC_URL
  - env://RUSTOK_API_URL
status: verified
---

# SEO public origin configuration

The SEO runtime uses a validated public origin when it renders canonical sitemap URLs, sitemap indexes, robots previews, and sitemap-submission payloads.

## Resolution order

The first non-empty value wins:

1. the tenant domain from `TenantContext`;
2. `RUSTOK_PUBLIC_URL`;
3. `RUSTOK_API_URL`.

There is no implicit localhost or other default. When all three values are missing, SEO operations that require an absolute public URL fail with a configuration error before a sitemap job is created.

## Accepted format

The value must represent an origin, not an application route:

```text
https://store.example.com
```

A value without a scheme is normalized to HTTPS:

```text
store.example.com -> https://store.example.com
```

The validator rejects:

- URL credentials;
- paths other than `/`;
- query strings and fragments;
- missing hosts;
- localhost, loopback, private-address literals, and local or internal hostnames;
- schemes other than HTTP and HTTPS.

Hosts are lowercased, a trailing DNS dot is removed, and a trailing URL slash is omitted from the canonical value.

## Operational guidance

Production deployments should set a tenant domain whenever each tenant has its own public host. Use `RUSTOK_PUBLIC_URL` for a shared externally visible platform origin. `RUSTOK_API_URL` remains a compatibility fallback and must still be an externally valid origin when SEO relies on it.

Do not use an internal service address, container hostname, proxy upstream, or browser-only development URL as the SEO public origin.

## Failure behavior

Invalid or missing configuration is reported as `SeoError::Configuration`. The runtime does not silently generate localhost URLs and does not create a running sitemap job before origin validation succeeds.

## Implementation

The contract is enforced in `crates/rustok-seo/src/services/sitemaps.rs` by the `PublicOrigin` parser and resolver.
