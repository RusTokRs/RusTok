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
import { type NextRequest, NextResponse } from "next/server";

export async function GET(request: NextRequest) {
  const { searchParams } = request.nextUrl;
  const secret = searchParams.get("secret");
  const slug = searchParams.get("slug");
  const locale = searchParams.get("locale") || "en";

  const expectedSecret =
    process.env.REVALIDATE_SECRET ||
    process.env.PREVIEW_SECRET ||
    "rustok-revalidate-secret";

  if (!secret || secret !== expectedSecret) {
    return NextResponse.json(
      { error: "Invalid preview secret" },
      { status: 401 },
    );
  }

  const draft = await draftMode();
  draft.enable();

  const targetPath = slug ? `/${locale}/pages/${encodeURIComponent(slug)}` : `/${locale}`;
  redirect(targetPath);
}
