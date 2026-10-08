'use client';

import { useState, useTransition } from 'react';
import type { GqlOpts, WorkflowVersionSummary } from '../api/workflows';
import { restoreWorkflowVersion } from '../api/workflows';

interface VersionHistoryProps {
  workflowId: string;
  versions: WorkflowVersionSummary[];
  opts: GqlOpts;
  onRestored?: () => void;
}

export function VersionHistory({
  workflowId,
  versions,
  opts,
  onRestored
}: VersionHistoryProps) {
  if (versions.length === 0) {
    return (
      <p className='text-muted-foreground text-sm'>No saved versions yet.</p>
    );
  }

  return (
    <div className='divide-border border-border divide-y overflow-hidden rounded-xl border'>
      {versions.map((v) => (
        <VersionRow
          key={v.version}
          workflowId={workflowId}
          version={v}
          opts={opts}
          onRestored={onRestored}
        />
      ))}
    </div>
  );
}

function VersionRow({
  workflowId,
  version,
  opts,
  onRestored
}: {
  workflowId: string;
  version: WorkflowVersionSummary;
  opts: GqlOpts;
  onRestored?: () => void;
}) {
  const [pending, startTransition] = useTransition();
  const [error, setError] = useState<string | null>(null);

  function handleRestore() {
    setError(null);
    startTransition(async () => {
      try {
        await restoreWorkflowVersion(workflowId, version.version, opts);
        onRestored?.();
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Restore failed');
      }
    });
  }

  return (
    <div className='hover:bg-muted/30 flex items-center justify-between px-4 py-3 transition-colors'>
      <div className='flex items-center gap-3'>
        <span className='text-foreground font-mono text-xs font-semibold'>
          v{version.version}
        </span>
        <span className='text-muted-foreground text-xs'>
          {new Date(version.createdAt).toLocaleString()}
        </span>
      </div>
      <div className='flex items-center gap-2'>
        {error && <span className='text-destructive text-xs'>{error}</span>}
        <button
          onClick={handleRestore}
          disabled={pending}
          className='hover:bg-muted border-border text-foreground rounded border px-2.5 py-1 text-xs font-medium transition disabled:opacity-50'
        >
          {pending ? '…' : 'Restore'}
        </button>
      </div>
    </div>
  );
}
