'use client';

import { useRouter } from 'next/navigation';
import { useState, useTransition } from 'react';

import type { FormSubmissionState, GqlOpts } from '../api/forms';
import { updateFormSubmissionState } from '../api/forms';

const NEXT_STATES: FormSubmissionState[] = ['read', 'handled', 'spam', 'new'];

export function SubmissionActions({
  opts,
  submissionId,
  state
}: {
  opts: GqlOpts;
  submissionId: string;
  state: FormSubmissionState;
}) {
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const [error, setError] = useState<string | null>(null);

  return (
    <span>
      {NEXT_STATES.filter((next) => next !== state).map((next) => (
        <button
          key={next}
          type="button"
          disabled={pending}
          onClick={() => {
            setError(null);
            startTransition(async () => {
              try {
                await updateFormSubmissionState(submissionId, next, opts);
                router.refresh();
              } catch (cause) {
                setError(cause instanceof Error ? cause.message : 'Update failed');
              }
            });
          }}
        >
          Mark {next}
        </button>
      ))}
      {error ? <span role="alert"> {error}</span> : null}
    </span>
  );
}
