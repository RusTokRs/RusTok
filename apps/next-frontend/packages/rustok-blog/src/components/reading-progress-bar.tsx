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
import React, { useEffect, useState } from 'react';
import { ArrowUp } from 'lucide-react';

export function ReadingProgressBar({
}: {
  locale?: string;
}): React.JSX.Element {
  const t = useTranslations('Blog');
  const [progress, setProgress] = useState(0);
  const [showScrollTop, setShowScrollTop] = useState(false);

  useEffect(() => {
    function handleScroll() {
      const totalHeight = document.documentElement.scrollHeight - window.innerHeight;
      if (totalHeight > 0) {
        const currentProgress = (window.scrollY / totalHeight) * 100;
        setProgress(Math.min(100, Math.max(0, currentProgress)));
      }
      setShowScrollTop(window.scrollY > 300);
    }

    window.addEventListener('scroll', handleScroll, { passive: true });
    handleScroll();

    return () => {
      window.removeEventListener('scroll', handleScroll);
    };
  }, []);

  const scrollToTop = () => {
    window.scrollTo({
      top: 0,
      behavior: 'smooth',
    });
  };

  return (
    <>
      {/* Top Reading Progress Bar */}
      <div
        className="fixed top-0 left-0 right-0 h-1 bg-muted/40 z-50 pointer-events-none"
        role="progressbar"
        aria-valuenow={Math.round(progress)}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <svg
          aria-hidden="true"
          className="block h-full w-full"
          preserveAspectRatio="none"
          viewBox="0 0 100 4"
        >
          <rect
            className="fill-primary transition-all duration-100 ease-out"
            x="0"
            y="0"
            width={`${progress}`}
            height="4"
          />
        </svg>
      </div>

      {/* Floating Scroll To Top Button */}
      {showScrollTop && (
        <Button
          type="button"
          onClick={scrollToTop}
          aria-label={t('article.scrollTop')}
          title={t('article.scrollTop')}
          className="fixed bottom-6 right-6 z-40 flex h-10 w-10 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg hover:bg-primary/90 hover:scale-105 active:scale-95 transition-all duration-200"
        >
          <ArrowUp className="h-5 w-5" />
        </Button>
      )}
    </>
  );
}
