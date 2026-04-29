import type { WorkflowExecution } from '../api/workflows';

interface ExecutionHistoryProps {
  executions: WorkflowExecution[];
}

export function ExecutionHistory({ executions }: ExecutionHistoryProps) {
  if (executions.length === 0) {
    return <p className='text-muted-foreground text-sm'>No executions yet.</p>;
  }

  return (
    <div className='rounded-md border'>
      <table className='w-full text-sm'>
        <thead>
          <tr className='bg-muted/50 border-b text-left'>
            <th className='px-4 py-2 font-medium'>Status</th>
            <th className='px-4 py-2 font-medium'>Started</th>
            <th className='px-4 py-2 font-medium'>Completed</th>
            <th className='px-4 py-2 font-medium'>Steps</th>
            <th className='px-4 py-2 font-medium'>Error</th>
          </tr>
        </thead>
        <tbody>
          {executions.map((exec) => (
            <tr
              key={exec.id}
              className='hover:bg-muted/30 border-b last:border-0'
            >
              <td className='px-4 py-2'>
                <span
                  className={`rounded-full px-2 py-0.5 text-xs font-medium ${statusClass(exec.status)}`}
                >
                  {exec.status}
                </span>
              </td>
              <td className='text-muted-foreground px-4 py-2'>
                {new Date(exec.startedAt).toLocaleString()}
              </td>
              <td className='text-muted-foreground px-4 py-2'>
                {exec.completedAt
                  ? new Date(exec.completedAt).toLocaleString()
                  : '—'}
              </td>
              <td className='px-4 py-2'>{exec.stepExecutions.length}</td>
              <td className='max-w-xs truncate px-4 py-2 text-red-600'>
                {exec.error ?? '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function statusClass(status: string): string {
  switch (status) {
    case 'COMPLETED':
      return 'bg-green-100 text-green-700';
    case 'FAILED':
      return 'bg-red-100 text-red-700';
    case 'RUNNING':
      return 'bg-blue-100 text-blue-700';
    case 'TIMED_OUT':
      return 'bg-orange-100 text-orange-700';
    default:
      return 'bg-gray-100 text-gray-500';
  }
}
