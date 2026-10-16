/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import React from "react";

export interface DraftPreviewBannerProps {
  slug: string;
  locale?: string;
  exitUrl?: string;
}

export function DraftPreviewBanner({
  slug,
  locale = "en",
  exitUrl,
}: DraftPreviewBannerProps) {
  const disableUrl =
    exitUrl ??
    `/api/disable-draft?redirect=${encodeURIComponent(`/${locale}/pages/${slug}`)}`;

  return (
    <aside
      aria-label="Draft Preview Mode"
      className="sticky top-0 z-50 flex items-center justify-between border-b border-amber-500/30 bg-amber-950/90 px-4 py-2 text-amber-200 backdrop-blur-md"
      data-fly-preview-banner="true"
    >
      <div className="flex items-center gap-2 text-xs md:text-sm font-medium">
        <span
          className="inline-block h-2.5 w-2.5 rounded-full bg-amber-400 animate-pulse"
          aria-hidden="true"
        />
        <span>
          <strong>Draft Preview Mode</strong> &mdash; Viewing unpublished or live draft for{" "}
          <code className="rounded bg-amber-900/60 px-1 py-0.5 text-amber-100">{slug}</code>
        </span>
      </div>
      <a
        href={disableUrl}
        className="rounded bg-amber-500/20 px-3 py-1 text-xs font-semibold text-amber-100 transition-colors hover:bg-amber-500/30 hover:text-white"
      >
        Exit Preview
      </a>
    </aside>
  );
}
