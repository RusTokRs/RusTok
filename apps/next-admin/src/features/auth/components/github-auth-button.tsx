'use client';

import { Button } from '@/shared/ui/shadcn/button';
import { Icons } from '@/shared/ui/icons';

export default function GithubSignInButton() {
  return (
    <Button
      className='w-full'
      variant='outline'
      type='button'
      onClick={() => console.log('continue with github clicked')}
    >
      <Icons.github className='mr-2 h-4 w-4' />
      Continue with Github
    </Button>
  );
}
