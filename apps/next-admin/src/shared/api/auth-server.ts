import 'server-only';

import { cookies, headers } from 'next/headers';
import {
  RUSTOK_ACCESS_COOKIE_NAME,
  RUSTOK_ACCESS_COOKIE_PATH,
  RUSTOK_ACCESS_REFRESH_SKEW_MS,
  RUSTOK_SESSION_COOKIE_NAME,
  RUSTOK_SESSION_COOKIE_PATH,
  RUSTOK_SESSION_REQUEST_HEADER,
  encodeRustokRefreshCredential,
  parseRustokHostCookies,
  parseRustokSessionCookie,
  retryAmbiguousRefresh,
  type RustokHostCookies,
  type RustokRefreshCredential
} from './auth-session';

const API_BASE_URL = (
  process.env.RUSTOK_API_URL ??
  process.env.NEXT_PUBLIC_API_URL ??
  'http://127.0.0.1:5150'
).replace(/\/+$/, '');

const REQUEST_TIMEOUT_MS = 10_000;
const LOGOUT_TIMEOUT_MS = 5_000;

export interface RustokSessionUser {
  id: string;
  email: string;
  name: string | null;
  role: string;
  status: string;
}

export interface RustokSessionCredentials {
  accessToken: string;
  expiresIn: number;
  refreshCredential: RustokRefreshCredential;
  refreshMaxAge: number;
  user: RustokSessionUser;
}

export class RustokSessionError extends Error {
  constructor(
    public readonly status: number,
    public readonly outcome: 'rejected' | 'unknown',
    message: string
  ) {
    super(message);
    this.name = 'RustokSessionError';
  }
}

interface SessionResponse {
  access_token: string;
  expires_in: number;
  user: RustokSessionUser;
}

function parseSessionResponse(value: unknown): SessionResponse {
  if (
    typeof value !== 'object' ||
    value === null ||
    Array.isArray(value)
  ) {
    throw new Error('Invalid session response');
  }

  const data = value as Record<string, unknown>;
  const user = data.user;
  if (
    typeof user !== 'object' ||
    user === null ||
    Array.isArray(user)
  ) {
    throw new Error('Invalid session response');
  }

  const record = user as Record<string, unknown>;
  if (
    typeof data.access_token !== 'string' ||
    data.access_token.length === 0 ||
    typeof data.expires_in !== 'number' ||
    !Number.isSafeInteger(data.expires_in) ||
    data.expires_in <= 0 ||
    typeof record.id !== 'string' ||
    record.id.length === 0 ||
    typeof record.email !== 'string' ||
    typeof record.role !== 'string' ||
    typeof record.status !== 'string' ||
    (record.name !== null && typeof record.name !== 'string')
  ) {
    throw new Error('Invalid session response');
  }

  return {
    access_token: data.access_token,
    expires_in: data.expires_in,
    user: {
      id: record.id,
      email: record.email,
      name: typeof record.name === 'string' ? record.name : null,
      role: record.role,
      status: record.status
    }
  };
}

function copyLocaleHeaders(source: Headers, target: Headers): void {
  for (const name of ['x-rustok-effective-locale', 'accept-language']) {
    const value = source.get(name);
    if (value) target.set(name, value);
  }
}

function toCredentials(
  response: SessionResponse,
  cookie: { credential: RustokRefreshCredential; maxAge: number }
): RustokSessionCredentials {
  return {
    accessToken: response.access_token,
    expiresIn: response.expires_in,
    refreshCredential: cookie.credential,
    refreshMaxAge: cookie.maxAge,
    user: response.user
  };
}

async function requestSession(
  path: string,
  init: RequestInit
): Promise<{
  response: Response;
  data: SessionResponse;
}> {
  let response: Response;
  try {
    response = await fetch(API_BASE_URL + path, {
      ...init,
      cache: 'no-store',
      redirect: 'error',
      signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS)
    });
  } catch {
    throw new RustokSessionError(
      0,
      'unknown',
      'Authentication service outcome could not be confirmed'
    );
  }

  if (!response.ok) {
    throw new RustokSessionError(
      response.status,
      response.status >= 400 && response.status < 500
        ? 'rejected'
        : 'unknown',
      'Authentication request failed'
    );
  }

  try {
    return { response, data: parseSessionResponse(await response.json()) };
  } catch {
    throw new RustokSessionError(
      502,
      'unknown',
      'Authentication service returned an invalid session'
    );
  }
}

export async function createRustokSession(input: {
  action: 'login' | 'register';
  email: string;
  password: string;
  tenantSlug: string;
  name?: string;
  request: Request;
}): Promise<RustokSessionCredentials> {
  const headers = new Headers({
    'content-type': 'application/json',
    'x-tenant-slug': input.tenantSlug
  });
  copyLocaleHeaders(input.request.headers, headers);

  const userAgent = input.request.headers.get('user-agent');
  if (userAgent) headers.set('user-agent', userAgent);

  const { response, data } = await requestSession(
    '/api/auth/session/' + input.action,
    {
      method: 'POST',
      headers,
      body: JSON.stringify({
        email: input.email,
        password: input.password,
        ...(input.action === 'register' ? { name: input.name ?? null } : {})
      })
    }
  );

  const cookie = parseRustokSessionCookie(
    response.headers.get('set-cookie')
  );
  if (!cookie) {
    throw new RustokSessionError(
      502,
      'unknown',
      'Rustok did not issue a valid refresh cookie'
    );
  }

  return toCredentials(data, cookie);
}

async function refreshOnce(
  credential: RustokRefreshCredential,
  tenantSlug: string,
  tenantId?: string | null,
  source?: Headers
): Promise<RustokSessionCredentials> {
  const headers = new Headers({
    cookie: RUSTOK_SESSION_COOKIE_NAME + '=' + encodeRustokRefreshCredential(credential),
    [RUSTOK_SESSION_REQUEST_HEADER]: '1',
    'x-tenant-slug': tenantSlug
  });
  if (tenantId) headers.set('x-tenant-id', tenantId);
  if (source) copyLocaleHeaders(source, headers);

  const { response, data } = await requestSession(
    '/api/auth/session/refresh',
    { method: 'POST', headers }
  );

  const cookie = parseRustokSessionCookie(
    response.headers.get('set-cookie')
  );
  if (!cookie || cookie.credential.retrySeed !== credential.retrySeed) {
    throw new RustokSessionError(
      502,
      'unknown',
      'Rustok did not return a matching refresh envelope'
    );
  }

  return toCredentials(data, cookie);
}

/** Repeat at most once after an ambiguous upstream result. The original
 * credential and proof remain unchanged; backend history is the distributed
 * authority, not a process-local promise map. Never retry a rejected request.
 */
export async function refreshRustokSession(
  credential: RustokRefreshCredential,
  tenantSlug: string,
  tenantId?: string | null,
  source?: Headers
): Promise<RustokSessionCredentials> {
  return retryAmbiguousRefresh(
    () => refreshOnce(credential, tenantSlug, tenantId, source),
    (error) => error instanceof RustokSessionError && error.outcome === 'unknown'
  );
}

export async function revokeRustokSession(
  credential: RustokRefreshCredential,
  tenantSlug: string,
  tenantId?: string | null
): Promise<void> {
  const headers = new Headers({
    cookie: RUSTOK_SESSION_COOKIE_NAME + '=' + encodeRustokRefreshCredential(credential),
    [RUSTOK_SESSION_REQUEST_HEADER]: '1',
    'x-tenant-slug': tenantSlug
  });
  if (tenantId) headers.set('x-tenant-id', tenantId);

  let response: Response;
  try {
    response = await fetch(API_BASE_URL + '/api/auth/session/logout', {
      method: 'POST',
      headers,
      cache: 'no-store',
      redirect: 'error',
      signal: AbortSignal.timeout(LOGOUT_TIMEOUT_MS)
    });
  } catch {
    throw new RustokSessionError(
      0,
      'unknown',
      'Logout outcome could not be confirmed'
    );
  }

  if (!response.ok) {
    throw new RustokSessionError(
      response.status,
      response.status < 500 ? 'rejected' : 'unknown',
      'Rustok logout failed'
    );
  }
  try {
    const body: unknown = await response.json();
    if (!body || typeof body !== 'object' || !('status' in body) || body.status !== 'ok') {
      throw new Error('Invalid logout response');
    }
  } catch {
    throw new RustokSessionError(502, 'unknown', 'Logout could not be confirmed');
  }
}

function cookieOptions(path: string) {
  return {
    httpOnly: true,
    sameSite: 'lax' as const,
    secure: process.env.NODE_ENV === 'production',
    path
  };
}

export async function persistRustokSessionCookies(
  session: RustokSessionCredentials
): Promise<void> {
  const store = await cookies();
  // Retain the exact Auth-issued envelope, not a second BFF proof protocol.
  store.set(RUSTOK_SESSION_COOKIE_NAME, encodeRustokRefreshCredential(session.refreshCredential), {
    ...cookieOptions(RUSTOK_SESSION_COOKIE_PATH),
    maxAge: session.refreshMaxAge
  });
  store.set(RUSTOK_ACCESS_COOKIE_NAME, session.accessToken, {
    ...cookieOptions(RUSTOK_ACCESS_COOKIE_PATH),
    maxAge: Math.max(
      1,
      session.expiresIn -
        Math.ceil(RUSTOK_ACCESS_REFRESH_SKEW_MS / 1000)
    )
  });
}

export async function readRustokSessionCookies(): Promise<RustokHostCookies> {
  return parseRustokHostCookies((await headers()).get('cookie'));
}

export async function clearRustokSessionCookies(): Promise<void> {
  const store = await cookies();
  store.set(RUSTOK_SESSION_COOKIE_NAME, '', {
    ...cookieOptions(RUSTOK_SESSION_COOKIE_PATH),
    maxAge: 0,
    expires: new Date(0)
  });
  store.set(RUSTOK_ACCESS_COOKIE_NAME, '', {
    ...cookieOptions(RUSTOK_ACCESS_COOKIE_PATH),
    maxAge: 0,
    expires: new Date(0)
  });
}
