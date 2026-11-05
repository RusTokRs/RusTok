'use client';
import { Button } from '@/shared/ui/base/button';

/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { useTranslations } from '@rustok/next-fluent';
import React, { useState } from 'react';
import { Check, Copy, Share2 } from 'lucide-react';

export function BlogShareButtons({
  title,
  url,
}: {
  title: string;
  url: string;
  locale?: string;
}): React.JSX.Element {
  const t = useTranslations('Blog');
  const [copied, setCopied] = useState(false);

  const shareTitle = encodeURIComponent(title);
  const shareUrl = encodeURIComponent(url);

  const telegramUrl = `https://t.me/share/url?url=${shareUrl}&text=${shareTitle}`;
  const vkUrl = `https://vk.com/share.php?url=${shareUrl}&title=${shareTitle}`;
  const twitterUrl = `https://twitter.com/intent/tweet?url=${shareUrl}&text=${shareTitle}`;

  const copyToClipboard = async () => {
    try {
      if (typeof window !== 'undefined') {
        const fullUrl = url.startsWith('http') ? url : `${window.location.origin}${url}`;
        await navigator.clipboard.writeText(fullUrl);
        setCopied(true);
        setTimeout(() => setCopied(false), 2500);
      }
    } catch {
      // fallback if clipboard api fails
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2 pt-4 border-t border-border">
      <span className="inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground mr-1">
        <Share2 className="h-3.5 w-3.5 text-primary" />
        <span>{t('share.title')}</span>
      </span>

      <a
        href={telegramUrl}
        target="_blank"
        rel="noopener noreferrer"
        className="rounded-lg border border-border bg-card px-2.5 py-1 text-xs font-medium text-foreground hover:bg-muted transition"
      >
        Telegram
      </a>

      <a
        href={vkUrl}
        target="_blank"
        rel="noopener noreferrer"
        className="rounded-lg border border-border bg-card px-2.5 py-1 text-xs font-medium text-foreground hover:bg-muted transition"
      >
        ВКонтакте
      </a>

      <a
        href={twitterUrl}
        target="_blank"
        rel="noopener noreferrer"
        className="rounded-lg border border-border bg-card px-2.5 py-1 text-xs font-medium text-foreground hover:bg-muted transition"
      >
        X (Twitter)
      </a>

      <Button
        type="button"
        onClick={copyToClipboard}
        className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-card px-2.5 py-1 text-xs font-medium text-foreground hover:bg-muted transition cursor-pointer"
      >
        {copied ? (
          <>
            <Check className="h-3.5 w-3.5 text-green-500" />
            <span className="text-green-600 dark:text-green-400 font-semibold">
              {t('share.copied')}
            </span>
          </>
        ) : (
          <>
            <Copy className="h-3.5 w-3.5 text-muted-foreground" />
            <span>{t('share.copy')}</span>
          </>
        )}
      </Button>
    </div>
  );
}
