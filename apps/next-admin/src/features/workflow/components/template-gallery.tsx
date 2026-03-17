'use client';

import { useState, useTransition } from 'react';
import { useRouter } from 'next/navigation';
import type { GqlOpts, WorkflowTemplate } from '../api/workflows';
import { createWorkflowFromTemplate } from '../api/workflows';

interface TemplateGalleryProps {
  templates: WorkflowTemplate[];
  opts: GqlOpts;
}

export function TemplateGallery({ templates, opts }: TemplateGalleryProps) {
  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold">Marketplace Templates</h2>
        <p className="text-sm text-muted-foreground">
          Pick a ready-made workflow to get started quickly.
        </p>
      </div>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {templates.map((t) => (
          <TemplateCard key={t.id} template={t} opts={opts} />
        ))}
      </div>
    </div>
  );
}

const CATEGORY_COLORS: Record<string, string> = {
  content: 'bg-blue-100 text-blue-700',
  commerce: 'bg-green-100 text-green-700',
  auth: 'bg-purple-100 text-purple-700',
  reporting: 'bg-yellow-100 text-yellow-700',
  integrations: 'bg-orange-100 text-orange-700',
};

function TemplateCard({ template, opts }: { template: WorkflowTemplate; opts: GqlOpts }) {
  const router = useRouter();
  const [name, setName] = useState('');
  const [pending, startTransition] = useTransition();
  const [error, setError] = useState<string | null>(null);

  const categoryColor = CATEGORY_COLORS[template.category] ?? 'bg-muted text-muted-foreground';

  function handleUse() {
    const workflowName = name.trim() || `My ${template.name}`;
    setError(null);
    startTransition(async () => {
      try {
        const id = await createWorkflowFromTemplate(template.id, workflowName, opts);
        router.push(`/dashboard/workflows/${id}`);
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Failed to create workflow');
      }
    });
  }

  return (
    <div className="flex flex-col gap-2 rounded-xl border border-border bg-card p-4">
      <div>
        <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${categoryColor}`}>
          {template.category}
        </span>
      </div>
      <h3 className="text-sm font-semibold leading-tight">{template.name}</h3>
      <p className="line-clamp-2 text-xs text-muted-foreground">{template.description}</p>
      {error && (
        <p className="text-xs text-destructive">{error}</p>
      )}
      <div className="mt-auto flex gap-2 pt-2">
        <input
          type="text"
          placeholder="Workflow name…"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="min-w-0 flex-1 rounded-lg border border-input bg-background px-2 py-1 text-xs"
        />
        <button
          onClick={handleUse}
          disabled={pending}
          className="rounded-lg bg-primary px-3 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {pending ? '…' : 'Use'}
        </button>
      </div>
    </div>
  );
}
