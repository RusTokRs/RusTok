/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

"use client";

import "@/styles/globals.css";

/**
 * Replaces the root layout on a fatal render error, so it must bring its own
 * html/body and stylesheet. React attaches the retry handler through a
 * nonce-bearing framework script; no inline event or style attribute is used.
 */
export default function GlobalError({
  retry,
}: {
  error: unknown;
  retry: () => void;
}) {
  // Do not echo server/client exception details into a public error document.
  const message = "We could not complete your request.";

  return (
    <html lang="en">
      <body className="min-h-screen bg-background text-foreground">
        <main className="flex min-h-screen flex-col items-center justify-center px-4 py-16 text-center">
          <div className="max-w-md space-y-6">
            <h1 className="text-2xl font-bold tracking-tight text-foreground">
              Something went wrong
            </h1>
            <p className="text-sm text-muted-foreground">{message}</p>
            <button
              className="inline-flex items-center rounded-md border border-border px-4 py-2 text-sm text-foreground"
              onClick={() => retry()}
              type="button"
            >
              Try again
            </button>
          </div>
        </main>
      </body>
    </html>
  );
}
