import { AlertTriangle } from 'lucide-react';

export function ModuleUnavailable({
  title = 'Module unavailable',
  description = 'This module is disabled for the current tenant.'
}: {
  title?: string;
  description?: string;
}) {
  return (
    <div className='border-border bg-card text-card-foreground flex min-h-[240px] flex-col items-center justify-center rounded-xl border border-dashed p-8 text-center'>
      <div className='bg-muted mb-4 rounded-full p-3'>
        <AlertTriangle className='text-muted-foreground h-5 w-5' />
      </div>
      <h2 className='text-lg font-semibold'>{title}</h2>
      <p className='text-muted-foreground mt-2 max-w-md text-sm'>
        {description}
      </p>
    </div>
  );
}
