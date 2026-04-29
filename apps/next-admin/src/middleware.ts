import { auth } from '@/auth';
import { EFFECTIVE_LOCALE_HEADER, resolveLocale } from '@/i18n/request';
import { NextResponse } from 'next/server';
import type { NextRequest } from 'next/server';

function resolveEffectiveLocale(req: NextRequest): string {
  const requestedLocale =
    req.nextUrl.searchParams.get('locale') ??
    req.headers.get(EFFECTIVE_LOCALE_HEADER) ??
    req.headers.get('accept-language')?.split(',')[0]?.split(';')[0]?.trim();

  return resolveLocale(requestedLocale);
}

export default auth((req) => {
  const { nextUrl, auth: session } = req;
  const isAuthenticated = !!session;
  const requestHeaders = new Headers(req.headers);
  requestHeaders.set(EFFECTIVE_LOCALE_HEADER, resolveEffectiveLocale(req));

  // Защищённые маршруты
  if (nextUrl.pathname.startsWith('/dashboard')) {
    if (!isAuthenticated) {
      const signInUrl = new URL('/auth/sign-in', nextUrl.origin);
      signInUrl.searchParams.set('callbackUrl', nextUrl.pathname);
      return NextResponse.redirect(signInUrl);
    }
  }

  // Корневой редирект
  if (nextUrl.pathname === '/') {
    return NextResponse.redirect(
      new URL(
        isAuthenticated ? '/dashboard/overview' : '/auth/sign-in',
        nextUrl.origin
      )
    );
  }

  return NextResponse.next({
    request: {
      headers: requestHeaders
    }
  });
});

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico|api/auth).*)']
};
