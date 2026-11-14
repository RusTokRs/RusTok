/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/**
 * Request-local CSP nonce and strict policy for the Next storefront host.
 *
 * The proxy issues one unpredictable nonce per request and forwards it to the
 * renderer in `CSP_NONCE_HEADER`. Next.js also parses the CSP request header
 * and applies the same nonce to framework scripts and framework-owned inline
 * styles. Server Components read the explicit header because framework-owned
 * tags are not the only inline elements the storefront emits: published landing
 * CSS and JSON-LD scripts are host-owned and must carry the nonce themselves.
 *
 * This module is framework-neutral: it uses only Web Crypto and Web Headers so
 * it can be imported by both the Edge proxy and Server Components.
 */

/** Request header carrying the nonce from the proxy to the renderer. */
export const CSP_NONCE_HEADER = "x-nonce";

/** Header carrying the policy that Next.js parses to nonce framework assets. */
export const CSP_POLICY_HEADER = "Content-Security-Policy";

/**
 * CSP3 `base64-value` characters, bounded so a malformed or attacker-supplied
 * header cannot inject a directive into `nonce-…` or an HTML attribute.
 */
const NONCE_PATTERN = /^(?=.{8,128}$)[A-Za-z0-9+/_-]+={0,2}$/;

/** One unpredictable, request-local nonce with 128 bits of Web Crypto entropy. */
export function createCspNonce(): string {
  const crypto = globalThis.crypto;
  if (!crypto?.getRandomValues) {
    throw new Error("Web Crypto getRandomValues is required to issue a CSP nonce");
  }

  const bytes = crypto.getRandomValues(new Uint8Array(16));
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return globalThis.btoa(binary);
}

export function isValidCspNonce(value: string | null | undefined): value is string {
  return typeof value === "string" && NONCE_PATTERN.test(value);
}

const NON_DOCUMENT_FETCH_DESTINATIONS = new Set([
  "audio",
  "embed",
  "font",
  "image",
  "manifest",
  "object",
  "script",
  "serviceworker",
  "sharedworker",
  "style",
  "track",
  "video",
  "worker",
]);

/**
 * Identifies file-like subresource requests so the host can leave them out of
 * locale negotiation and per-document CSP/no-store handling. HTML navigations
 * and RSC (`text/x-component`) requests keep going through the proxy, including
 * when their pathname contains a dot.
 */
export function shouldBypassCspForNonDocumentResource(
  pathname: string,
  headers: Pick<Headers, "get">,
): boolean {
  const destination = headers.get("sec-fetch-dest")?.toLowerCase();
  if (destination && NON_DOCUMENT_FETCH_DESTINATIONS.has(destination)) return true;
  if (!/\\.[^/]+$/.test(pathname)) return false;

  const accept = headers.get("accept")?.toLowerCase() ?? "";
  return !accept.includes("text/html") && !accept.includes("text/x-component");
}

export interface StrictCspOptions {
  nonce: string;
  /** Development needs `unsafe-eval` for React/Turbopack diagnostics; production does not. */
  isDev?: boolean;
}

/**
 * Builds a single-line, nonce-only policy. Style attributes are refused
 * outright (`style-src-attr 'none'`), so host-owned inline styles must be
 * nonce-bearing `<style>` elements instead of `style="…"` attributes.
 *
 * `frame-ancestors 'self'` keeps the same-origin rich-text frame usable while
 * refusing third-party embedding. `img-src`/`media-src` allow HTTPS assets
 * because published landings can reference media on an external CDN.
 */
export function buildStrictCsp({ nonce, isDev = false }: StrictCspOptions): string {
  if (!isValidCspNonce(nonce)) {
    throw new Error("invalid CSP nonce");
  }

  const scriptSources = ["'self'", `'nonce-${nonce}'`, "'strict-dynamic'"];
  if (isDev) scriptSources.push("'unsafe-eval'");

  return (
    [
      "default-src 'self'",
      `script-src ${scriptSources.join(" ")}`,
      "script-src-attr 'none'",
      `style-src 'self' 'nonce-${nonce}'`,
      "style-src-attr 'none'",
      "img-src 'self' data: blob: https:",
      "font-src 'self' data:",
      "connect-src 'self'",
      "media-src 'self' data: blob: https:",
      "frame-src 'self'",
      "object-src 'none'",
      "base-uri 'self'",
      "form-action 'self'",
      "frame-ancestors 'self'",
    ].join("; ") + ";"
  );
}
