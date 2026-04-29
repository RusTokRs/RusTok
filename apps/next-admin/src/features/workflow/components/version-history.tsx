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
    <div className='rounded-md border'>
      <table className='w-full text-sm'>
        <thead>
          <tr className='bg-muted/50 border-b text-left'>
            <th className='px-4 py-2 font-medium'>Version</th>
            <th className='px-4 py-2 font-medium'>Saved</th>
            <th className='px-4 py-2 font-medium' />
          </tr>
        </thead>
        <tbody>
          {versions.map((v) => (
            <VersionRow
              key={v.version}
              workflowId={workflowId}
              version={v}
              opts={opts}
              onRestored={onRestored}
            />
          ))}
        </tbody>
      </table>
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
    <tr className='hover:bg-muted/30 border-b last:border-0'>
      <td className='px-4 py-2 font-mono text-xs'>v{version.version}</td>
      <td className='text-muted-foreground px-4 py-2'>
        {new Date(version.createdAt).toLocaleString()}
      </td>
      <td className='px-4 py-2 text-right'>
        {error && (
          <span className='text-destructive mr-2 text-xs'>{error}</span>
        )}
        <button
          onClick={handleRestore}
          disabled={pending}
          className='hover:bg-muted rounded border px-2 py-1 text-xs disabled:opacity-50'
        >
          {pending ? '…' : 'Restore'}
        </button>
      </td>
    </tr>
  );
}
