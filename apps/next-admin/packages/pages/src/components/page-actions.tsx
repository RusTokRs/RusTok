'use client';

import { useRouter } from 'next/navigation';
import { useState, useTransition } from 'react';

import type { GqlOpts } from '../api/pages';
import { createPage, deletePage, unpublishPage } from '../api/pages';

interface PageActionsProps {
  opts: GqlOpts;
  pageId?: string;
  status?: string;
  /** Locale used for newly created pages. */
  locale?: string;
}

/**
 * Row/create actions for the Pages surface. Visual editing and reviewed publish
 * intentionally live in the Rust FFA builder; here we only create drafts and
 * reverse publication through the same GraphQL both admins consume.
 */
export function PageActions({ opts, pageId, status, locale = 'en' }: PageActionsProps) {
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const [error, setError] = useState<string | null>(null);
  const [title, setTitle] = useState('');

  function run(action: () => Promise<unknown>) {
    setError(null);
    startTransition(async () => {
      try {
        await action();
        router.refresh();
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : 'Request failed');
      }
    });
  }

  if (!pageId) {
    return (
      <form
        onSubmit={(event) => {
          event.preventDefault();
          const value = title.trim();
          if (!value) return;
          run(async () => {
            await createPage({ locale, title: value }, opts);
            setTitle('');
          });
        }}
      >
        <label>
          New page title{' '}
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="Landing page"
          />
        </label>{' '}
        <button type="submit" disabled={pending || !title.trim()}>
          Create draft
        </button>
        {error ? <p role="alert">{error}</p> : null}
      </form>
    );
  }

  return (
    <span>
      {status === 'published' ? (
        <button
          type="button"
          disabled={pending}
          onClick={() => run(() => unpublishPage(pageId, undefined, opts))}
        >
          Unpublish
        </button>
      ) : null}{' '}
      <button
        type="button"
        disabled={pending}
        onClick={() => {
          if (window.confirm('Delete this page permanently?')) {
            run(() => deletePage(pageId, opts));
          }
        }}
      >
        Delete
      </button>
      {error ? (
        <span role="alert"> {error}</span>
      ) : null}
    </span>
  );
}
