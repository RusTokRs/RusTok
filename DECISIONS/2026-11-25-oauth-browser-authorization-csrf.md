# OAuth Browser Authorization CSRF Boundary

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: Implemented
- Owners: platform / auth
- Extends: [OAuth Access-Token Revocation Fence](./2026-11-01-oauth-access-token-revocation-fence.md)
- Supersedes: None
- Superseded by: None

## Context

The hosted OAuth browser flow stores the direct-user access token in an
HttpOnly `SameSite=Lax` cookie scoped to `/api/oauth`. Lax cookies are sent on
cross-site top-level safe-method navigations, including GET requests to
`/api/oauth/authorize`.

The browser authorization handler previously issued a code directly from GET
when the app did not require user consent or when an active consent already
covered the requested scopes. A third-party page could therefore navigate a
signed-in user to a registered client authorization URL and cause a code to be
issued for that user without a user-confirmed action. The attacker-controlled
client could supply its own redirect URI, state and PKCE challenge, then redeem
the resulting code.

The existing HMAC CSRF token protected only the form used when consent was
missing. It did not protect the two GET fast paths.

## Decision

1. A browser authorization GET authenticated by the ambient browser-session
   cookie is read-only. It must always render the authorization confirmation
   form and must never issue an authorization code, regardless of app type or
   existing consent.
2. The hosted form carries a short-lived HMAC CSRF token bound to tenant,
   direct user, client id, registered redirect URI, optional scope, optional
   state and PKCE challenge. Only a POST to the consent/confirmation endpoint
   with a valid token can issue the code for a cookie-authenticated flow.
3. The POST rechecks active consent against the requested scopes. It grants
   consent only when the app requires user consent and the existing active
   grant does not cover the request. Apps that do not require consent do not
   receive synthetic consent rows.
4. An explicit `Authorization: Bearer` credential is non-ambient and cannot
   be attached by an ordinary cross-site navigation. The existing fast path
   remains available only for that transport when consent is not required or
   already covers the request. Requests needing new consent still use the
   CSRF-bound form.
5. The browser-session cookie remains HttpOnly and SameSite=Lax so legitimate
   top-level authorization navigation continues to work. CSRF protection is
   provided by the mandatory form/POST boundary, not by changing cookie
   SameSite semantics.

## Invariants

- Cookie-authenticated GET never inserts an authorization-code row and never
  redirects with a code.
- A code for a cookie-authenticated browser flow is issued only after a valid,
  unexpired HMAC confirmation token is verified against the current direct
  user and exact request parameters.
- A previously active consent does not bypass user confirmation for a
  cookie-authenticated GET.
- A first-party or embedded app that does not require consent still requires
  explicit confirmation when authentication is supplied only by the browser
  cookie; no fake consent row is persisted.
- A stale or revoked consent cannot authorize a code: POST rechecks consent and
  the code-issuance owner revalidates the authority fence inside its
  transaction.
- Delegated OAuth and service principals remain unable to authorize or
  consent.

## Failure semantics

- Missing or invalid browser-session credentials render the existing
  authorization-required response.
- Missing, expired or mismatched CSRF confirmation returns
  `invalid_request` and issues no code.
- Consent lookup or persistence failures return a sanitized `server_error`.
- Code issuance continues to use the transactional app/user/consent fences
  defined by the OAuth token lifecycle owner.

## Verification

- Unit coverage for cookie-vs-explicit-bearer confirmation policy.
- Existing HMAC binding and consent epoch tests.
- Maintainer-run server OAuth tests and full verification remain required.

## Consequences

Cookie-backed browser authorization now requires an explicit confirmation
click even when the user has previously consented or the app type does not
require consent. This is an intentional security boundary. Non-ambient bearer
clients retain their existing pre-authorized fast path.
