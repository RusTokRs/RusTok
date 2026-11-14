/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { getRequestCspNonce } from "@/shared/security/nonce";

/**
 * Root 404 for unmatched URLs. Next's built-in 404 uses inline `style`
 * attributes and an unnonced style block, both of which a strict,
 * nonce-only CSP refuses. Staying with external stylesheet classes keeps the
 * 404 response compliant without weakening the policy for ordinary pages.
 */
export default async function NotFound() {
  // A nonce is one-use; `headers()` opts this document into dynamic rendering
  // so Next can apply the request nonce to its framework scripts.
  await getRequestCspNonce();

  return (
    <main className="flex min-h-[60vh] flex-col items-center justify-center px-4 py-16 text-center">
      <div className="max-w-md space-y-6">
        <p className="text-8xl font-black text-foreground sm:text-9xl">404</p>
        <div className="space-y-2">
          <h1 className="text-2xl font-bold tracking-tight text-foreground sm:text-3xl">
            Page not found
          </h1>
          <p className="text-sm text-muted-foreground">
            The requested page does not exist or is temporarily unavailable.
          </p>
        </div>
        <a
          className="inline-flex items-center rounded-md border border-border px-4 py-2 text-sm text-foreground"
          href="/"
        >
          Return to the storefront
        </a>
      </div>
    </main>
  );
}
