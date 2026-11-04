'use client';

import Link from 'next/link';
import { Home, Search } from 'lucide-react';
import { Button } from '@/shared/ui/base/button';

export default function NotFound() {
  return (
    <div className='flex min-h-[60vh] flex-col items-center justify-center px-4 py-16 text-center'>
      <div className='max-w-md space-y-6'>
        <span className='bg-gradient-to-b from-foreground to-muted-foreground/30 bg-clip-text text-8xl font-black text-transparent sm:text-9xl'>
          404
        </span>
        <div className='space-y-2'>
          <h2 className='text-2xl font-bold tracking-tight text-foreground sm:text-3xl'>
            Page not found
          </h2>
          <p className='text-sm text-muted-foreground'>
            Sorry, the page you are looking for doesn&apos;t exist, was removed, or is temporarily unavailable.
          </p>
        </div>
        <div className='flex flex-wrap items-center justify-center gap-3 pt-2'>
          <Button asChild size='sm' variant='outline'>
            <Link href='/'>
              <Home className='mr-2 h-4 w-4' />
              Storefront Home
            </Link>
          </Button>
          <Button asChild size='sm'>
            <Link href='/search'>
              <Search className='mr-2 h-4 w-4' />
              Search Catalog
            </Link>
          </Button>
        </div>
      </div>
    </div>
  );
}

