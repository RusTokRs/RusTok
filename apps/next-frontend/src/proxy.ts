/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { createI18nMiddleware } from "@rustok/next-fluent/middleware";
import { NextResponse, type NextRequest } from "next/server";

import { defaultLocale, locales } from "./i18n-config";
import {
  CSP_NONCE_HEADER,
  CSP_POLICY_HEADER,
  buildStrictCsp,
  createCspNonce,
  shouldBypassCspForNonDocumentResource,
} from "./shared/security/csp";

const i18n = createI18nMiddleware({
  locales,
  defaultLocale,
});

type I18nRequest = Parameters<typeof i18n>[0];

/**
 * Projects the Next request onto the minimal request contract the i18n
 * middleware declares. It copies the headers (including the new nonce/CSP
 * headers) into the response request override, so the renderer receives them.
 */
function i18nRequest(request: NextRequest, headers: Headers): I18nRequest {
  return {
    url: request.url,
    nextUrl: {
      pathname: request.nextUrl.pathname,
      search: request.nextUrl.search,
      basePath: request.nextUrl.basePath,
    },
    cookies: {
      get: (name: string) => {
        const value = request.cookies.get(name)?.value;
        return value === undefined ? undefined : { value };
      },
    },
    headers,
  };
}

/**
 * Strict CSP is enforced for every production HTML response. Development can
 * opt in with `RUSTOK_STRICT_CSP=1`; otherwise the nonce header is still
 * forwarded so host-owned inline elements render identically without an
 * enforcing policy that would fight Turbopack's development runtime.
 */
function strictCspEnabled(): boolean {
  return (
    process.env.NODE_ENV === "production" ||
    process.env.RUSTOK_STRICT_CSP === "1"
  );
}

export async function proxy(request: NextRequest): Promise<NextResponse> {
  if (
    shouldBypassCspForNonDocumentResource(
      request.nextUrl.pathname,
      request.headers,
    )
  ) {
    return NextResponse.next();
  }

  const nonce = createCspNonce();
  const csp = strictCspEnabled()
    ? buildStrictCsp({ nonce, isDev: process.env.NODE_ENV !== "production" })
    : null;

  const requestHeaders = new Headers(request.headers);
  requestHeaders.set(CSP_NONCE_HEADER, nonce);
  // Next.js parses this request header to nonce framework scripts and
  // framework-owned inline styles with the same value.
  if (csp) requestHeaders.set(CSP_POLICY_HEADER, csp);

  const response = await i18n(i18nRequest(request, requestHeaders));
  if (csp) {
    response.headers.set(CSP_POLICY_HEADER, csp);
    // A nonce is one-use: a cached HTML document would replay it to a
    // different request and defeat the policy.
    response.headers.set("Cache-Control", "private, no-store");
  }
  return response;
}

export const config = {
  // Do not exclude every dotted pathname: dynamic slugs and HTML 404s still
  // need the document policy. Resource fetches are bypassed above by request
  // destination/Accept instead of by guessing from the pathname alone.
  matcher: ["/", "/((?!api(?:/|$)|_next(?:/|$)|_vercel(?:/|$)).*)"],
};
