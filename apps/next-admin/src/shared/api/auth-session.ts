export const RUSTOK_SESSION_COOKIE_NAME = 'rustok-session';
export const RUSTOK_ACCESS_COOKIE_NAME = 'rustok-access';
export const RUSTOK_SESSION_COOKIE_PATH = '/api';
export const RUSTOK_ACCESS_COOKIE_PATH = '/api/rustok';
export const RUSTOK_SESSION_REQUEST_HEADER = 'x-rustok-session-request';
export const RUSTOK_EXPECTED_SESSION_ID_HEADER = 'x-rustok-expected-session-id';
export const RUSTOK_ACCESS_REFRESH_SKEW_MS = 60_000;

const MAX_SESSION_COOKIE_AGE_SECONDS = 90 * 24 * 60 * 60;

/** Auth-owned HttpOnly envelope. The BFF relays both components unchanged. */
export interface RustokRefreshCredential {
  token: string;
  retrySeed: string;
}

function isSecret(value: string): boolean {
  return value.length === 64 && /^[0-9a-f]{64}$/.test(value) && value !== '0'.repeat(64);
}

export function encodeRustokRefreshCredential(credential: RustokRefreshCredential): string {
  if (!isSecret(credential.token) || !isSecret(credential.retrySeed)) {
    throw new Error('Invalid refresh credential');
  }
  return credential.token + '.' + credential.retrySeed;
}

export function parseRustokRefreshCredential(value: string): RustokRefreshCredential | null {
  const parts = value.split('.');
  if (parts.length !== 2 || !isSecret(parts[0]) || !isSecret(parts[1])) return null;
  return { token: parts[0], retrySeed: parts[1] };
}

export interface RustokHostCookies {
  accessToken: string | null;
  refreshCredential: RustokRefreshCredential | null;
  invalid: boolean;
}

/** Inspect the raw Cookie header, not a framework map that may discard
 * duplicate names. Never select credentials by their order in the header.
 */
export function parseRustokHostCookies(header: string | null): RustokHostCookies {
  const access: string[] = [];
  const refresh: string[] = [];
  for (const pair of (header ?? '').split(';')) {
    const separator = pair.indexOf('=');
    const name = (separator < 0 ? pair : pair.slice(0, separator)).trim();
    const value = separator < 0 ? '' : pair.slice(separator + 1).trim();
    if (name === RUSTOK_ACCESS_COOKIE_NAME) access.push(value);
    if (name === RUSTOK_SESSION_COOKIE_NAME) refresh.push(value);
  }
  const credential = refresh.length === 1 ? parseRustokRefreshCredential(refresh[0]) : null;
  const invalid = access.length > 1 || refresh.length > 1 ||
    (refresh.length === 1 && credential === null) ||
    (access.length === 1 && (!credential || !access[0] || /[\s,]/.test(access[0])));
  return {
    accessToken: !invalid && access.length === 1 ? access[0] : null,
    refreshCredential: !invalid ? credential : null,
    invalid
  };
}

/** Only the exact same refresh attempt may be repeated, at most once. */
export async function retryAmbiguousRefresh<T>(
  attempt: () => Promise<T>,
  isAmbiguous: (error: unknown) => boolean
): Promise<T> {
  try {
    return await attempt();
  } catch (error) {
    if (!isAmbiguous(error)) throw error;
    return attempt();
  }
}

export interface ParsedRustokSessionCookie {
  credential: RustokRefreshCredential;
  maxAge: number;
}

export function parseRustokSessionCookie(
  setCookie: string | null
): ParsedRustokSessionCookie | null {
  if (!setCookie) return null;

  const [pair, ...attributes] = setCookie.split(';');
  const separator = pair.indexOf('=');
  if (
    separator < 1 ||
    pair.slice(0, separator).trim() !== RUSTOK_SESSION_COOKIE_NAME
  ) {
    return null;
  }

  const value = pair.slice(separator + 1).trim();
  const credential = parseRustokRefreshCredential(value);
  if (!credential) return null;

  const maxAgeAttributes = attributes.filter((part) =>
    /^\s*max-age=/i.test(part)
  );
  if (maxAgeAttributes.length !== 1) return null;
  const maxAgeAttribute = maxAgeAttributes[0];
  const rawAge = maxAgeAttribute.slice(maxAgeAttribute.indexOf('=') + 1).trim();
  if (!/^[1-9][0-9]*$/.test(rawAge)) return null;
  const maxAge = Number(rawAge);
  if (
    !Number.isSafeInteger(maxAge) ||
    maxAge < 1 ||
    maxAge > MAX_SESSION_COOKIE_AGE_SECONDS
  ) {
    return null;
  }

  return { credential, maxAge };
}

export function isCanonicalSessionId(value: unknown): value is string {
  return typeof value === 'string' && value.length === 36
    && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value)
    && value !== '00000000-0000-0000-0000-000000000000';
}

/** Compare trusted response projections, not authenticate a JWT. The backend
 * remains the only authority for its signature, expiry and live session.
 */
export function rustokDirectSessionClaims(accessToken: string): {
  sessionId: string; userId: string; tenantId: string;
} | null {
  try {
    const parts = accessToken.split('.');
    if (parts.length !== 3 || parts.some(part => !/^[A-Za-z0-9_-]+$/.test(part))) return null;
    const claims = JSON.parse(Buffer.from(parts[1], 'base64url').toString('utf8'));
    if (claims?.grant_type !== 'direct' || !isCanonicalSessionId(claims.session_id)
      || !isCanonicalSessionId(claims.sub) || !isCanonicalSessionId(claims.tenant_id)) return null;
    return { sessionId: claims.session_id, userId: claims.sub, tenantId: claims.tenant_id };
  } catch {
    return null;
  }
}

export function matchesRustokDirectSession(
  token: string, sessionId: string, userId: string, tenantId: string | null | undefined
): boolean {
  const claims = rustokDirectSessionClaims(token);
  return claims !== null && claims.sessionId === sessionId && claims.userId === userId
    && claims.tenantId === tenantId;
}

/**
 * The backend verifies the JWT. Its decoded expiry is used only to schedule
 * refresh before the access cookie expires.
 */
export function isRustokAccessTokenExpiring(
  accessToken: string,
  nowMs = Date.now()
): boolean {
  try {
    const parts = accessToken.split('.');
    if (parts.length !== 3 || !parts[1]) return true;

    const claims = JSON.parse(
      Buffer.from(parts[1], 'base64url').toString('utf8')
    ) as { exp?: unknown };

    return (
      typeof claims.exp !== 'number' ||
      !Number.isSafeInteger(claims.exp) ||
      claims.exp <= 0 ||
      claims.exp * 1000 <= nowMs + RUSTOK_ACCESS_REFRESH_SKEW_MS
    );
  } catch {
    return true;
  }
}

export function isSameOriginRequest(
  requestUrl: string,
  origin: string | null,
  referer: string | null
): boolean {
  try {
    const expectedOrigin = new URL(requestUrl).origin;
    const source = origin ?? referer;
    return source !== null && new URL(source).origin === expectedOrigin;
  } catch {
    return false;
  }
}
