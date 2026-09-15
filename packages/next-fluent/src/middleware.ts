import type { I18nMiddlewareOptions } from './types';
import { matchSupportedLocale, resolveAcceptLanguage } from './utils';

export interface NextMiddlewareRequestLike {
  url: string;
  nextUrl: {
    pathname: string;
    search: string;
  };
  cookies: {
    get(name: string): { value: string } | undefined;
    set?(name: string, value: string, options?: unknown): void;
  };
  headers: {
    get(name: string): string | null;
  };
}

export function createI18nMiddleware(options: I18nMiddlewareOptions) {
  const {
    locales,
    defaultLocale,
    cookieName = 'rustok-locale',
    headerName = 'x-rustok-effective-locale',
  } = options;

  return async function middleware(request: NextMiddlewareRequestLike) {
    let NextResponse: any;

    try {
      const nextServer = await import('next/server');
      NextResponse = nextServer.NextResponse;
    } catch {
      NextResponse = class MockNextResponse {
        static next() {
          const headers = new Headers();
          return {
            status: 200,
            headers,
            cookies: {
              set: (name: string, val: string) => headers.append('Set-Cookie', `${name}=${val}; Path=/`),
            },
          };
        }
        static redirect(url: URL | string) {
          const headers = new Headers();
          headers.set('location', String(url));
          return {
            status: 307,
            headers,
            cookies: {
              set: (name: string, val: string) => headers.append('Set-Cookie', `${name}=${val}; Path=/`),
            },
          };
        }
      };
    }

    const { pathname, search } = request.nextUrl;
    const segments = pathname.split('/').filter(Boolean);
    const firstSegment = segments[0];

    const matchedPrefix = matchSupportedLocale(firstSegment, locales);

    if (matchedPrefix) {
      const response = NextResponse.next();
      response.headers.set(headerName, matchedPrefix);
      if (response.cookies?.set) {
        response.cookies.set(cookieName, matchedPrefix, { path: '/' });
      }
      return response;
    }

    const cookieLocale = matchSupportedLocale(
      request.cookies.get(cookieName)?.value ||
        request.cookies.get('rustok-admin-locale')?.value ||
        request.cookies.get('rustok-frontend-locale')?.value ||
        request.cookies.get('NEXT_LOCALE')?.value,
      locales
    );

    const headerLocale = resolveAcceptLanguage(
      request.headers.get('accept-language'),
      locales
    );

    const targetLocale = cookieLocale || headerLocale || defaultLocale;

    const targetPath = `/${targetLocale}${pathname === '/' ? '' : pathname}${search}`;
    const targetUrl = new URL(targetPath, request.url);

    const response = NextResponse.redirect(targetUrl);
    response.headers.set(headerName, targetLocale);
    if (response.cookies?.set) {
      response.cookies.set(cookieName, targetLocale, { path: '/' });
    }
    return response;
  };
}

export const createMiddleware = createI18nMiddleware;
