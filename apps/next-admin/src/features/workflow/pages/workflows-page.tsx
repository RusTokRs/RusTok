import Link from 'next/link';
import type { GqlOpts, WorkflowSummary } from '../api/workflows';
import { listWorkflows, listWorkflowTemplates } from '../api/workflows';
import { TemplateGallery } from '../components/template-gallery';
import { ModuleUnavailable } from '@/shared/ui/module-unavailable';

interface WorkflowsPageProps {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export default async function WorkflowsPage({
  token,
  tenantSlug,
  tenantId
}: WorkflowsPageProps) {
  const opts: GqlOpts = { token, tenantSlug, tenantId };
  let workflowLoadError: string | null = null;
  const [workflows, templates] = await Promise.all([
    listWorkflows(opts).catch((error) => {
      workflowLoadError =
        error instanceof Error ? error.message : 'Failed to load workflows';
      return [] as WorkflowSummary[];
    }),
    listWorkflowTemplates(opts).catch(() => [])
  ]);

  if (workflowLoadError) {
    return (
      <ModuleUnavailable
        title='Workflow module is unavailable'
        description={workflowLoadError}
      />
    );
  }

  return (
    <div className='space-y-8'>
      <div className='space-y-4'>
        <div className='flex items-center justify-between'>
          <h1 className='text-2xl font-bold'>Workflows</h1>
          <Link
            href='/dashboard/workflows/new'
            className='bg-primary text-primary-foreground rounded px-4 py-2 text-sm font-medium'
          >
            New Workflow
          </Link>
        </div>
        <div className='rounded-md border'>
          <table className='w-full text-sm'>
            <thead>
              <tr className='bg-muted/50 border-b text-left'>
                <th className='px-4 py-3 font-medium'>Name</th>
                <th className='px-4 py-3 font-medium'>Status</th>
                <th className='px-4 py-3 font-medium'>Failures</th>
                <th className='px-4 py-3 font-medium'>Updated</th>
                <th className='px-4 py-3 font-medium' />
              </tr>
            </thead>
            <tbody>
              {workflows.length === 0 ? (
                <tr>
                  <td
                    colSpan={5}
                    className='text-muted-foreground px-4 py-8 text-center'
                  >
                    No workflows yet.
                  </td>
                </tr>
              ) : (
                workflows.map((wf) => (
                  <tr
                    key={wf.id}
                    className='hover:bg-muted/30 border-b last:border-0'
                  >
                    <td className='px-4 py-3 font-medium'>{wf.name}</td>
                    <td className='px-4 py-3'>
                      <span
                        className={`rounded-full px-2 py-0.5 text-xs font-medium ${statusClass(wf.status)}`}
                      >
                        {wf.status}
                      </span>
                    </td>
                    <td className='px-4 py-3'>{wf.failureCount}</td>
                    <td className='text-muted-foreground px-4 py-3'>
                      {new Date(wf.updatedAt).toLocaleDateString()}
                    </td>
                    <td className='px-4 py-3 text-right'>
                      <Link
                        href={`/dashboard/workflows/${wf.id}`}
                        className='text-primary hover:underline'
                      >
                        View
                      </Link>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {templates.length > 0 && (
        <section className='border-border rounded-xl border p-6'>
          <TemplateGallery templates={templates} opts={opts} />
        </section>
      )}
    </div>
  );
}

function statusClass(status: string): string {
  switch (status) {
    case 'ACTIVE':
      return 'bg-green-100 text-green-700';
    case 'PAUSED':
      return 'bg-yellow-100 text-yellow-700';
    case 'ARCHIVED':
      return 'bg-gray-100 text-gray-500';
    default:
      return 'bg-blue-100 text-blue-700';
  }
}
