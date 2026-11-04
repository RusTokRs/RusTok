'use client';

import { useEffect } from 'react';
import Link from 'next/link';
import { AlertTriangle, RefreshCcw, Home } from 'lucide-react';
import { Button } from '@/shared/ui/base/button';

export default function LocaleError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error('Storefront error:', error);
  }, [error]);

  return (
    <div className='flex min-h-[50vh] flex-col items-center justify-center px-4 py-16 text-center'>
      <div className='max-w-md space-y-6'>
        <div className='mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-destructive/10 text-destructive'>
          <AlertTriangle className='h-6 w-6' />
        </div>
        <div className='space-y-2'>
          <h2 className='text-2xl font-bold tracking-tight text-foreground'>
            Something went wrong
          </h2>
          <p className='text-sm text-muted-foreground'>
            {error.message || 'We could not complete your request. Please try again.'}
          </p>
        </div>
        <div className='flex flex-wrap items-center justify-center gap-3'>
          <Button variant='outline' size='sm' onClick={() => reset()}>
            <RefreshCcw className='mr-2 h-4 w-4' />
            Try again
          </Button>
          <Button asChild size='sm'>
            <Link href='/'>
              <Home className='mr-2 h-4 w-4' />
              Go to Home
            </Link>
          </Button>
        </div>
      </div>
    </div>
  );
}
