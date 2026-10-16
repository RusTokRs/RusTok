/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { revalidatePath, revalidateTag } from "next/cache";
import { type NextRequest, NextResponse } from "next/server";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json().catch(() => ({}));
    const secret =
      body.secret ??
      request.nextUrl.searchParams.get("secret") ??
      request.headers.get("x-revalidate-secret");

    const configuredSecret = process.env.REVALIDATE_SECRET;
    const isDev = process.env.NODE_ENV !== "production";
    const expectedSecret = configuredSecret || (isDev ? "rustok-revalidate-secret" : undefined);

    if (!expectedSecret || !secret || secret !== expectedSecret) {
      return NextResponse.json(
        { error: "Invalid revalidation secret" },
        { status: 401 },
      );
    }

    const tags: string[] = [];
    if (body.tag && typeof body.tag === "string") {
      tags.push(body.tag);
    }
    if (Array.isArray(body.tags)) {
      for (const t of body.tags) {
        if (typeof t === "string") tags.push(t);
      }
    }
    const queryTag = request.nextUrl.searchParams.get("tag");
    if (queryTag) tags.push(queryTag);

    const paths: string[] = [];
    if (body.path && typeof body.path === "string") {
      paths.push(body.path);
    }
    if (Array.isArray(body.paths)) {
      for (const p of body.paths) {
        if (typeof p === "string") paths.push(p);
      }
    }
    const queryPath = request.nextUrl.searchParams.get("path");
    if (queryPath) paths.push(queryPath);

    if (tags.length === 0 && paths.length === 0) {
      tags.push("pages");
    }

    for (const tag of tags) {
      revalidateTag(tag, "max");
    }

    for (const path of paths) {
      revalidatePath(path);
    }

    return NextResponse.json({
      revalidated: true,
      tags,
      paths,
      now: Date.now(),
    });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : "Revalidation failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function GET(request: NextRequest) {
  return POST(request);
}
