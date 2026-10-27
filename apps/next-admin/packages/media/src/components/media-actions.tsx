'use client';

import { useRouter } from 'next/navigation';
import { useState, useTransition } from 'react';

import type { GqlOpts } from '../api/media';
import { deleteMedia } from '../api/media';

export function MediaActions({
  opts,
  mediaId
}: {
  opts: GqlOpts;
  mediaId: string;
}) {
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const [error, setError] = useState<string | null>(null);

  return (
    <span>
      <button
        type='button'
        disabled={pending}
        onClick={() => {
          if (!window.confirm('Delete this media asset permanently?')) return;
          setError(null);
          startTransition(async () => {
            try {
              await deleteMedia(mediaId, opts);
              router.refresh();
            } catch (cause) {
              setError(
                cause instanceof Error ? cause.message : 'Delete failed'
              );
            }
          });
        }}
      >
        Delete
      </button>
      {error ? <span role='alert'> {error}</span> : null}
    </span>
  );
}
