# Customer Admin client transport error safety

Status: **source-ready / unvalidated**

## Scope

This source slice covers the public Customer Admin transport facade in
`crates/rustok-customer/admin/src/transport/mod.rs`.

Covered operations:

- `fetch_bootstrap`
- `fetch_customers`
- `fetch_customer_detail`
- `create_customer`
- `update_customer`

## Confirmed gap

The private native adapter converts `ServerFnError` into
`ApiError::ServerFn(value.to_string())`. The public facade previously re-exported
that private error type and returned it unchanged, so framework, transport,
serialization, or unexpected server-function text could become UI-visible.

## Source policy

The facade now exports a separate public `ApiError` whose `ServerFn` variant has
no string payload. Its `Display` implementation always returns:

`Customer admin request could not be completed`

Each facade operation creates a `CustomerAdminTransportErrorContext` before the
unchanged native adapter call and maps only the final private native error.

The original error remains only in structured diagnostics with:

- owner, exact facade operation, correlation id, boundary, and stable code;
- customer-id and search presence/character length;
- pagination and payload presence without their values.

Customer ids, search text, pagination values, email, names, phone, locale,
metadata, and draft contents are not written by the client transport mapper.

## Preserved behavior

This slice does not change:

- the private native adapter or mounted server-function endpoints;
- authentication, tenant, permission, and profile-audience policy;
- customer owner calls or request order;
- request and response DTOs;
- Customer Admin native-only transport selection.

The existing server-side native policy remains independent and unchanged.

## Evidence boundary

The retained JSON evidence is source-only. Tests, focused verifiers, Cargo,
formatting, workflows, CI, hydrate compilation, SSR compilation, browser behavior,
and mounted runtime failure behavior were not executed by this implementation
agent.

The broad ecommerce mapper-cleanup item remains open for other owners and other
non-`PortError` public envelopes.
