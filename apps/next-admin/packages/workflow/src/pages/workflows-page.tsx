import type { GqlOpts, WorkflowSummary } from '../api/workflows';
import { listWorkflows, listWorkflowTemplates } from '../api/workflows';
import { TemplateGallery } from '../components/template-gallery';
import { WorkflowsTable } from '../components/workflows-table';
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
    <div className='space-y-6'>
      <div className='space-y-4'>
        <WorkflowsTable data={workflows} />
      </div>

      {templates.length > 0 && (
        <section className='bg-card border-border rounded-xl border p-6 shadow-sm'>
          <TemplateGallery templates={templates} opts={opts} />
        </section>
      )}
    </div>
  );
}
