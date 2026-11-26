import { auth } from '@/auth';
import {
  clearRustokSessionCookies,
  isRustokAccessTokenExpiring,
  persistRustokSessionCookies,
  readRustokSessionCookies,
  refreshRustokSession,
  RustokSessionError
} from '@/shared/api/auth-server';
import { isSameOriginRequest } from '@/shared/api/auth-session';

const API_BASE_URL = (
  process.env.RUSTOK_API_URL ??
  process.env.NEXT_PUBLIC_API_URL ??
  'http://127.0.0.1:5150'
).replace(/\/+$/, '');
const GRAPHQL_URL = API_BASE_URL + '/api/graphql';
const MAX_BODY_BYTES = 1024 * 1024;

function graphQLError(
  code: string,
  message: string,
  status = 200,
  outcome: 'rejected' | 'unknown' = 'rejected'
): Response {
  return Response.json(
    {
      errors: [{ message, extensions: { code, retryable: false, outcome } }]
    },
    { status, headers: { 'cache-control': 'no-store' } }
  );
}

async function readBoundedBody(request: Request): Promise<string | null> {
  const reader = request.body?.getReader();
  if (!reader) return '';

  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > MAX_BODY_BYTES) {
        await reader.cancel();
        return null;
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }

  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(bytes);
}

export async function POST(request: Request): Promise<Response> {
  if (
    !isSameOriginRequest(
      request.url,
      request.headers.get('origin'),
      request.headers.get('referer')
    )
  ) {
    return graphQLError(
      'FORBIDDEN',
      'Cross-origin GraphQL requests are not allowed',
      403
    );
  }

  const contentType = (request.headers.get('content-type') ?? '')
    .split(';', 1)[0]
    .trim()
    .toLowerCase();
  if (contentType !== 'application/json') {
    return graphQLError(
      'BAD_USER_INPUT',
      'GraphQL requests must use application/json',
      415
    );
  }

  const declaredLength = Number(request.headers.get('content-length') ?? 0);
  if (
    Number.isFinite(declaredLength) &&
    declaredLength > MAX_BODY_BYTES
  ) {
    return graphQLError(
      'PAYLOAD_TOO_LARGE',
      'GraphQL request body is too large',
      413
    );
  }

  const body = await readBoundedBody(request);
  if (body === null) {
    return graphQLError(
      'PAYLOAD_TOO_LARGE',
      'GraphQL request body is too large',
      413
    );
  }

  const session = await auth();
  const stored = await readRustokSessionCookies();
  const upstreamHeaders = new Headers({
    'content-type': 'application/json'
  });

  if (session?.user) {
    const tenantSlug = session.user.tenantSlug;
    const tenantId = session.user.tenantId;
    const requestedTenantSlug = request.headers.get('x-tenant-slug');
    const requestedTenantId = request.headers.get('x-tenant-id');

    if (
      !tenantSlug ||
      (requestedTenantSlug !== null &&
        requestedTenantSlug !== tenantSlug)
    ) {
      return graphQLError(
        'FORBIDDEN',
        'Workspace does not match the authenticated session',
        403
      );
    }

    if (
      tenantId &&
      requestedTenantId !== null &&
      requestedTenantId !== tenantId
    ) {
      return graphQLError(
        'FORBIDDEN',
        'Tenant does not match the authenticated session',
        403
      );
    }

    if (!stored.accessToken && !stored.refreshToken) {
      return graphQLError(
        'UNAUTHORIZED',
        'Rustok session is unavailable. Sign in again.',
        401
      );
    }

    let accessToken = stored.accessToken;
    if (!accessToken || isRustokAccessTokenExpiring(accessToken)) {
      if (!stored.refreshToken) {
        return graphQLError(
          'UNAUTHORIZED',
          'Rustok session expired. Sign in again.',
          401
        );
      }

      try {
        const refreshed = await refreshRustokSession(
          stored.refreshToken,
          tenantSlug,
          tenantId,
          request.headers
        );
        await persistRustokSessionCookies(refreshed);
        accessToken = refreshed.accessToken;
      } catch (error) {
        if (
          error instanceof RustokSessionError &&
          error.outcome === 'rejected'
        ) {
          await clearRustokSessionCookies();
          return graphQLError(
            'UNAUTHORIZED',
            'Rustok session could not be refreshed. Sign in again.',
            401
          );
        }

        return graphQLError(
          'SERVICE_UNAVAILABLE',
          'Session refresh outcome is unknown; inspect current state before retrying.',
          503,
          'unknown'
        );
      }
    }

    upstreamHeaders.set('authorization', 'Bearer ' + accessToken);
    upstreamHeaders.set('x-tenant-slug', tenantSlug);
    if (tenantId) upstreamHeaders.set('x-tenant-id', tenantId);
  } else {
    if (stored.accessToken || stored.refreshToken) {
      await clearRustokSessionCookies();
      return graphQLError(
        'UNAUTHORIZED',
        'Application session is unavailable. Sign in again.',
        401
      );
    }

    const tenantSlug = request.headers.get('x-tenant-slug');
    if (tenantSlug) upstreamHeaders.set('x-tenant-slug', tenantSlug);
  }

  for (const name of [
    'x-rustok-effective-locale',
    'accept-language'
  ]) {
    const value = request.headers.get(name);
    if (value) upstreamHeaders.set(name, value);
  }

  let upstream: Response;
  try {
    upstream = await fetch(GRAPHQL_URL, {
      method: 'POST',
      headers: upstreamHeaders,
      body,
      cache: 'no-store'
    });
  } catch {
    return graphQLError(
      'SERVICE_UNAVAILABLE',
      'GraphQL request outcome is unknown; inspect current state before retrying.',
      503,
      'unknown'
    );
  }

  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: {
      'cache-control': 'no-store',
      'content-type':
        upstream.headers.get('content-type') ?? 'application/json'
    }
  });
}
