export const RUSTOK_SESSION_COOKIE_NAME = 'rustok-session';
export const RUSTOK_ACCESS_COOKIE_NAME = 'rustok-access';
export const RUSTOK_SESSION_COOKIE_PATH = '/api';
export const RUSTOK_ACCESS_COOKIE_PATH = '/api/rustok';
export const RUSTOK_SESSION_REQUEST_HEADER = 'x-rustok-session-request';
export const RUSTOK_ACCESS_REFRESH_SKEW_MS = 60_000;

const MAX_SESSION_COOKIE_AGE_SECONDS = 90 * 24 * 60 * 60;

export function parseRustokSessionCookie(setCookie: string | null): {value: string; maxAge: number} | null {
  if (!setCookie) return null;
  const [pair, ...attributes] = setCookie.split(';');
  const separator = pair.indexOf('=');
  if (separator < 1 || pair.slice(0, separator).trim() !== RUSTOK_SESSION_COOKIE_NAME) return null;
  const value = pair.slice(separator + 1).trim();
  if (!/^[0-9a-f]{64}$/.test(value)) return null;
  const maxAgeAttribute = attributes.find(part => /^\s*max-age=/i.test(part));
  if (!maxAgeAttribute) return null;
  const maxAge = Number(maxAgeAttribute.slice(maxAgeAttribute.indexOf('=') + 1).trim());
  if (!Number.isSafeInteger(maxAge) || maxAge < 1 || maxAge > MAX_SESSION_COOKIE_AGE_SECONDS) return null;
  return {value, maxAge};
}

export function isRustokAccessTokenExpiring(accessToken: string, nowMs = Date.now()): boolean {
  try {
    const parts = accessToken.split('.');
    if (parts.length !== 3 || !parts[1]) return true;
    const claims = JSON.parse(Buffer.from(parts[1], 'base64url').toString('utf8')) as {exp?: unknown};
    return typeof claims.exp !== 'number' || !Number.isSafeInteger(claims.exp) ||
      claims.exp <= 0 || claims.exp * 1000 <= nowMs + RUSTOK_ACCESS_REFRESH_SKEW_MS;
  } catch { return true; }
}

export function isSameOriginRequest(requestUrl: string, origin: string | null, referer: string | null): boolean {
  try {
    const expected = new URL(requestUrl).origin;
    const source = origin ?? referer;
    return source !== null && new URL(source).origin === expected;
  } catch { return false; }
}
