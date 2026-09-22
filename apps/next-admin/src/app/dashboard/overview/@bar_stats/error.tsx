'use client';

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/shadcn/alert';
import { Button } from '@/shared/ui/shadcn/button';
import { Card, CardContent, CardHeader } from '@/shared/ui/shadcn/card';
import { IconAlertCircle } from '@tabler/icons-react';
import { useRouter } from 'next/navigation';
import { useEffect, useTransition } from 'react';

interface StatsErrorProps {
  error: Error;
  reset: () => void; // Add reset function from error boundary
}

async function captureException(error: Error) {
  if (process.env.NEXT_PUBLIC_SENTRY_DISABLED) return;

  const Sentry = await import('@sentry/nextjs');
  Sentry.captureException(error);
}

export default function StatsError({ error, reset }: StatsErrorProps) {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();

  useEffect(() => {
    void captureException(error);
  }, [error]);

  // the reload fn ensures the refresh is deferred  until the next render phase allowing react to handle any pending states before processing
  const reload = () => {
    startTransition(() => {
      router.refresh();
      reset();
    });
  };
  return (
    <Card className='border-red-500'>
      <CardHeader className='flex flex-col items-stretch space-y-0 border-b p-0 sm:flex-row'>
        <div className='flex flex-1 flex-col justify-center gap-1 px-6 py-5 sm:py-6'>
          <Alert variant='destructive' className='border-none'>
            <IconAlertCircle className='h-4 w-4' />
            <AlertTitle>Error</AlertTitle>
            <AlertDescription className='mt-2'>
              Failed to load statistics: {error.message}
            </AlertDescription>
          </Alert>
        </div>
      </CardHeader>
      <CardContent className='flex h-[316px] items-center justify-center p-6'>
        <div className='text-center'>
          <p className='text-muted-foreground mb-4 text-sm'>
            Unable to display statistics at this time
          </p>
          <Button
            onClick={() => reload()}
            variant='outline'
            className='min-w-[120px]'
            disabled={isPending}
          >
            Try again
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
