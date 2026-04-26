'use client';

import { useRouter } from 'next/navigation';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { toast } from 'sonner';
import type { GqlOpts, WorkflowResponse } from '../api/workflows';
import { createWorkflow, updateWorkflow } from '../api/workflows';

const schema = z.object({
  name: z.string().min(1, 'Name is required'),
  description: z.string().optional(),
  triggerType: z.enum(['manual', 'event', 'cron', 'webhook']),
  triggerEventType: z.string().optional(),
  triggerCron: z.string().optional()
});

type FormValues = z.infer<typeof schema>;

interface WorkflowFormPageProps {
  workflow?: WorkflowResponse;
  opts?: GqlOpts;
}

export default function WorkflowFormPage({ workflow, opts = {} }: WorkflowFormPageProps) {
  const router = useRouter();
  const isEdit = !!workflow;

  const defaultTriggerType = workflow
    ? ((workflow.triggerConfig as { type?: string }).type as FormValues['triggerType']) ?? 'manual'
    : 'manual';

  const { register, handleSubmit, watch, formState: { errors, isSubmitting } } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: workflow?.name ?? '',
      description: workflow?.description ?? '',
      triggerType: defaultTriggerType,
      triggerEventType: (workflow?.triggerConfig as { event_type?: string })?.event_type ?? '',
      triggerCron: (workflow?.triggerConfig as { expression?: string })?.expression ?? ''
    }
  });

  const triggerType = watch('triggerType');

  function buildTriggerConfig(values: FormValues): Record<string, unknown> {
    switch (values.triggerType) {
      case 'event':
        return { type: 'event', event_type: values.triggerEventType ?? '' };
      case 'cron':
        return { type: 'cron', expression: values.triggerCron ?? '0 * * * * *' };
      case 'webhook':
        return { type: 'webhook', path: `/hooks/${values.name.toLowerCase().replace(/\s+/g, '-')}` };
      default:
        return { type: 'manual' };
    }
  }

  async function onSubmit(values: FormValues) {
    try {
      const triggerConfig = buildTriggerConfig(values);
      if (isEdit && workflow) {
        await updateWorkflow(workflow.id, {
          name: values.name,
          description: values.description,
          triggerConfig
        }, opts);
        toast.success('Workflow updated');
        router.refresh();
      } else {
        const id = await createWorkflow({
          name: values.name,
          description: values.description,
          triggerConfig
        }, opts);
        toast.success('Workflow created');
        router.push(`/dashboard/workflows/${id}`);
      }
    } catch {
      toast.error(isEdit ? 'Failed to update workflow' : 'Failed to create workflow');
    }
  }

  return (
    <div className="max-w-lg space-y-6">
      <h1 className="text-2xl font-bold">{isEdit ? 'Edit Workflow' : 'New Workflow'}</h1>
      <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
        <div>
          <label className="mb-1 block text-sm font-medium">Name</label>
          <input
            {...register('name')}
            className="w-full rounded border px-3 py-2 text-sm"
            placeholder="My workflow"
          />
          {errors.name && <p className="mt-1 text-xs text-red-500">{errors.name.message}</p>}
        </div>

        <div>
          <label className="mb-1 block text-sm font-medium">Description</label>
          <textarea
            {...register('description')}
            className="w-full rounded border px-3 py-2 text-sm"
            rows={3}
            placeholder="Optional description"
          />
        </div>

        <div>
          <label className="mb-1 block text-sm font-medium">Trigger</label>
          <select {...register('triggerType')} className="w-full rounded border px-3 py-2 text-sm">
            <option value="manual">Manual</option>
            <option value="event">Domain Event</option>
            <option value="cron">Cron Schedule</option>
            <option value="webhook">Webhook</option>
          </select>
        </div>

        {triggerType === 'event' && (
          <div>
            <label className="mb-1 block text-sm font-medium">Event type</label>
            <input
              {...register('triggerEventType')}
              className="w-full rounded border px-3 py-2 text-sm font-mono"
              placeholder="blog.post.published"
            />
          </div>
        )}

        {triggerType === 'cron' && (
          <div>
            <label className="mb-1 block text-sm font-medium">Cron expression (6-field)</label>
            <input
              {...register('triggerCron')}
              className="w-full rounded border px-3 py-2 text-sm font-mono"
              placeholder="0 0 * * * *"
            />
          </div>
        )}

        <div className="flex gap-3 pt-2">
          <button
            type="submit"
            disabled={isSubmitting}
            className="rounded bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
          >
            {isSubmitting ? 'Saving...' : isEdit ? 'Save Changes' : 'Create Workflow'}
          </button>
          <button
            type="button"
            onClick={() => router.back()}
            className="rounded border px-4 py-2 text-sm hover:bg-muted"
          >
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}
