/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { draftMode } from "next/headers";
import { redirect } from "next/navigation";
import { type NextRequest } from "next/server";

export async function GET(request: NextRequest) {
  const { searchParams } = request.nextUrl;
  let redirectPath = "/";
  const rawRedirect = searchParams.get("redirect") || "/";

  // Prevent open redirect attacks: only allow valid same-origin relative paths
  try {
    const parsed = new URL(rawRedirect, request.nextUrl.origin);
    if (parsed.origin === request.nextUrl.origin && parsed.pathname.startsWith("/")) {
      redirectPath = parsed.pathname + parsed.search;
    }
  } catch {
    redirectPath = "/";
  }

  const draft = await draftMode();
  draft.disable();

  redirect(redirectPath);
}
