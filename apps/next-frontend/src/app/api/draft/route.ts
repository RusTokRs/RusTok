/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { timingSafeEqual } from "node:crypto";
import { draftMode } from "next/headers";
import { redirect } from "next/navigation";
import { type NextRequest, NextResponse } from "next/server";

function constantTimeCompare(a: string, b: string): boolean {
  const aBuf = Buffer.from(a);
  const bBuf = Buffer.from(b);
  if (aBuf.length !== bBuf.length) {
    return false;
  }
  return timingSafeEqual(aBuf, bBuf);
}

export async function GET(request: NextRequest) {
  const { searchParams } = request.nextUrl;
  const secret = searchParams.get("secret");
  const slug = searchParams.get("slug");
  const rawLocale = searchParams.get("locale") || "en";
  const locale = /^[a-z]{2}(-[a-zA-Z]{2,4})?$/.test(rawLocale) ? rawLocale : "en";

  const configuredSecret =
    process.env.REVALIDATE_SECRET || process.env.PREVIEW_SECRET;
  const isDev = process.env.NODE_ENV !== "production";
  const expectedSecret = configuredSecret || (isDev ? "rustok-revalidate-secret" : undefined);

  if (!expectedSecret || !secret || !constantTimeCompare(secret, expectedSecret)) {
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
