'use client';

import { useEffect } from 'react';
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/shadcn/alert';
import { Button } from '@/shared/ui/shadcn/button';
import { AlertTriangle, RefreshCcw } from 'lucide-react';

export default function DashboardError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error('Dashboard route error:', error);
  }, [error]);

  return (
    <div className='flex flex-1 flex-col items-center justify-center p-6'>
      <div className='w-full max-w-md space-y-4'>
        <Alert variant='destructive'>
          <AlertTriangle className='h-4 w-4' />
          <AlertTitle>An error occurred</AlertTitle>
          <AlertDescription className='mt-2 text-sm'>
            {error.message || 'Unable to load this section of the administration dashboard.'}
          </AlertDescription>
        </Alert>
        <div className='flex justify-end gap-2'>
          <Button variant='outline' size='sm' onClick={() => reset()}>
            <RefreshCcw className='mr-2 h-4 w-4' />
            Try again
          </Button>
        </div>
      </div>
    </div>
  );
}
