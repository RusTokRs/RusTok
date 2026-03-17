import Link from 'next/link';
import type { GqlOpts } from '../api/workflows';
import { getWorkflow, listWorkflowExecutions, listWorkflowVersions } from '../api/workflows';
import { WorkflowStepEditor } from '../components/workflow-step-editor';
import { ExecutionHistory } from '../components/execution-history';
import { VersionHistory } from '../components/version-history';

interface WorkflowDetailPageProps {
  id: string;
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export default async function WorkflowDetailPage({
  id,
  token,
  tenantSlug,
  tenantId
}: WorkflowDetailPageProps) {
  const opts: GqlOpts = { token, tenantSlug, tenantId };
  const [workflow, executions, versions] = await Promise.all([
    getWorkflow(id, opts),
    listWorkflowExecutions(id, opts),
    listWorkflowVersions(id, opts).catch(() => []),
  ]);

  if (!workflow) {
    return (
      <div className="py-16 text-center text-muted-foreground">
        Workflow not found.
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-bold">{workflow.name}</h1>
          {workflow.description && (
            <p className="mt-1 text-muted-foreground">{workflow.description}</p>
          )}
        </div>
        <div className="flex gap-2">
          <Link
            href={`/dashboard/workflows/${id}/edit`}
            className="rounded border px-3 py-1.5 text-sm hover:bg-muted"
          >
            Edit
          </Link>
          <span className={`rounded-full px-2 py-1 text-xs font-medium ${statusClass(workflow.status)}`}>
            {workflow.status}
          </span>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-4 text-sm">
        <div className="rounded border p-3">
          <div className="text-muted-foreground">Trigger</div>
          <div className="mt-1 font-mono text-xs">
            {(workflow.triggerConfig as { type?: string }).type ?? 'unknown'}
          </div>
        </div>
        <div className="rounded border p-3">
          <div className="text-muted-foreground">Steps</div>
          <div className="mt-1 font-semibold">{workflow.steps.length}</div>
        </div>
        <div className="rounded border p-3">
          <div className="text-muted-foreground">Failures</div>
          <div className="mt-1 font-semibold">{workflow.failureCount}</div>
        </div>
      </div>

      <section>
        <h2 className="mb-3 text-lg font-semibold">Steps</h2>
        <WorkflowStepEditor workflowId={id} steps={workflow.steps} opts={opts} />
      </section>

      <section>
        <h2 className="mb-3 text-lg font-semibold">Execution History</h2>
        <ExecutionHistory executions={executions} />
      </section>

      <section>
        <h2 className="mb-3 text-lg font-semibold">Version History</h2>
        <VersionHistory workflowId={id} versions={versions} opts={opts} />
      </section>
    </div>
  );
}

function statusClass(status: string): string {
  switch (status) {
    case 'ACTIVE': return 'bg-green-100 text-green-700';
    case 'PAUSED': return 'bg-yellow-100 text-yellow-700';
    case 'ARCHIVED': return 'bg-gray-100 text-gray-500';
    default: return 'bg-blue-100 text-blue-700';
  }
}
