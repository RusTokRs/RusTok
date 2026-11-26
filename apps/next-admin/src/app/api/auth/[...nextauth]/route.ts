import { authRouteAction } from '@/shared/api/auth-session';
import { confirmedSignOut, handlers } from '@/auth';
import type { NextRequest } from 'next/server';

type RouteContext = { params: Promise<{ nextauth: string[] }> };

function rejected(status: number): Response {
  return Response.json({ status: 'rejected' }, {
    status,
    headers: { 'cache-control': 'no-store' }
  });
}

export async function GET(request: NextRequest, context: RouteContext): Promise<Response> {
  const segments = (await context.params).nextauth;
  if (request.nextUrl.pathname !== '/api/auth/' + segments.join('/')) return rejected(404);
  const action = authRouteAction('GET', segments);
  if (action !== 'authjs') return rejected(action === 'logout' ? 405 : 404);
  return handlers.GET(request);
}

export async function POST(request: NextRequest, context: RouteContext): Promise<Response> {
  const segments = (await context.params).nextauth;
  if (request.nextUrl.pathname !== '/api/auth/' + segments.join('/')) return rejected(404);
  const action = authRouteAction('POST', segments);
  if (action === 'logout') return confirmedSignOut(request);
  if (action !== 'authjs') return rejected(404);
  return handlers.POST(request);
}
