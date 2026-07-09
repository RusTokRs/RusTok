'use client';

import { useState } from 'react';
import { toast } from 'sonner';
import type { GqlOpts, WorkflowStep, CreateStepInput } from '../api/workflows';
import { addWorkflowStep, deleteWorkflowStep } from '../api/workflows';

interface WorkflowStepEditorProps {
  workflowId: string;
  steps: WorkflowStep[];
  opts?: GqlOpts;
}

const STEP_TYPES = [
  'ACTION',
  'CONDITION',
  'DELAY',
  'ALLOY_SCRIPT',
  'EMIT_EVENT',
  'HTTP',
  'NOTIFY',
  'TRANSFORM'
] as const;

export function WorkflowStepEditor({
  workflowId,
  steps: initialSteps,
  opts = {}
}: WorkflowStepEditorProps) {
  const [steps, setSteps] = useState<WorkflowStep[]>(initialSteps);
  const [adding, setAdding] = useState(false);
  const [newStep, setNewStep] = useState<Partial<CreateStepInput>>({
    stepType: 'ACTION',
    onError: 'STOP',
    position: (initialSteps.length + 1) * 10
  });

  async function handleAdd() {
    if (!newStep.stepType) return;
    setAdding(true);
    try {
      await addWorkflowStep(
        workflowId,
        {
          position: newStep.position ?? (steps.length + 1) * 10,
          stepType: newStep.stepType,
          config: {},
          onError: newStep.onError ?? 'STOP'
        },
        opts
      );
      toast.success('Step added');
      // Optimistically refresh would need router.refresh(); for now show message
      setNewStep({
        stepType: 'ACTION',
        onError: 'STOP',
        position: (steps.length + 2) * 10
      });
    } catch {
      toast.error('Failed to add step');
    } finally {
      setAdding(false);
    }
  }

  async function handleDelete(stepId: string) {
    try {
      await deleteWorkflowStep(workflowId, stepId, opts);
      setSteps((prev) => prev.filter((s) => s.id !== stepId));
      toast.success('Step removed');
    } catch {
      toast.error('Failed to remove step');
    }
  }

  return (
    <div className='space-y-3'>
      {steps.length === 0 ? (
        <p className='text-muted-foreground text-sm'>
          No steps yet. Add one below.
        </p>
      ) : (
        <ol className='space-y-2'>
          {steps.map((step, idx) => (
            <li
              key={step.id}
              className='flex items-center gap-3 rounded border px-4 py-3 text-sm'
            >
              <span className='bg-muted flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium'>
                {idx + 1}
              </span>
              <span className='flex-1 font-mono'>{step.stepType}</span>
              <span className='text-muted-foreground text-xs'>
                on_error: {step.onError}
              </span>
              <button
                onClick={() => handleDelete(step.id)}
                className='text-xs text-red-500 hover:underline'
              >
                Remove
              </button>
            </li>
          ))}
        </ol>
      )}

      <div className='flex items-end gap-2 pt-2'>
        <div>
          <label className='text-muted-foreground mb-1 block text-xs font-medium'>
            Type
          </label>
          <select
            value={newStep.stepType}
            onChange={(e) =>
              setNewStep((p) => ({
                ...p,
                stepType: e.target.value as CreateStepInput['stepType']
              }))
            }
            className='rounded border px-2 py-1.5 text-sm'
          >
            {STEP_TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className='text-muted-foreground mb-1 block text-xs font-medium'>
            On Error
          </label>
          <select
            value={newStep.onError}
            onChange={(e) =>
              setNewStep((p) => ({
                ...p,
                onError: e.target.value as CreateStepInput['onError']
              }))
            }
            className='rounded border px-2 py-1.5 text-sm'
          >
            <option value='STOP'>Stop</option>
            <option value='SKIP'>Skip</option>
            <option value='RETRY'>Retry</option>
          </select>
        </div>
        <button
          onClick={handleAdd}
          disabled={adding}
          className='bg-primary text-primary-foreground rounded px-3 py-1.5 text-sm font-medium disabled:opacity-50'
        >
          {adding ? 'Adding…' : '+ Add Step'}
        </button>
      </div>
    </div>
  );
}
