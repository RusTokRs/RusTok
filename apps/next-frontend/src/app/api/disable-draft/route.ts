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
  let redirectPath = searchParams.get("redirect") || "/";

  // Prevent open redirect attacks: only allow valid relative paths without protocol or backslash
  if (
    !redirectPath.startsWith("/") ||
    redirectPath.startsWith("//") ||
    redirectPath.includes("\\") ||
    redirectPath.includes("\r") ||
    redirectPath.includes("\n")
  ) {
    redirectPath = "/";
  }

  const draft = await draftMode();
  draft.disable();

  redirect(redirectPath);
}
