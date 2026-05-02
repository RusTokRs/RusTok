'use client';

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from '@/shared/ui/shadcn/dropdown-menu';
import { Avatar, AvatarFallback } from '@/shared/ui/shadcn/avatar';
import {
  IconChevronDown,
  IconLogout,
  IconUserCircle
} from '@tabler/icons-react';
import { signOut, useSession } from 'next-auth/react';
import { useTranslations } from 'next-intl';
import { useRouter } from 'next/navigation';

function getInitials(value: string, fallback: string) {
  const trimmed = value.trim();
  if (!trimmed) return fallback;
  return trimmed.slice(0, 1).toUpperCase();
}

export function HeaderUserMenu() {
  const router = useRouter();
  const { data: session } = useSession();
  const tMenu = useTranslations('app.menu');
  const user = session?.user;
  const displayName = user?.name || user?.email || tMenu('defaultUser');
  const email = user?.email ?? '';
  const role = user?.role || 'user';
  const initial = getInitials(displayName, tMenu('userInitial'));

  const handleLogout = () => {
    signOut({ callbackUrl: '/auth/sign-in' });
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type='button'
          className='hover:bg-accent focus-visible:ring-ring flex items-center gap-2 rounded-lg p-2 transition-colors focus-visible:ring-2 focus-visible:outline-none'
          aria-label={tMenu('defaultUser')}
        >
          <Avatar className='h-8 w-8'>
            <AvatarFallback className='bg-primary text-primary-foreground text-sm font-semibold'>
              {initial}
            </AvatarFallback>
          </Avatar>
          <div className='hidden text-left md:block'>
            <p className='text-foreground text-sm leading-none font-medium'>
              {displayName}
            </p>
            <p className='text-muted-foreground mt-1 text-xs'>{role}</p>
          </div>
          <IconChevronDown className='text-muted-foreground size-4' />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align='end' className='w-56 rounded-lg'>
        <DropdownMenuLabel className='font-normal'>
          <div className='flex flex-col gap-1'>
            <p className='text-popover-foreground truncate text-sm font-medium'>
              {displayName}
            </p>
            {email && (
              <p className='text-muted-foreground truncate text-xs'>{email}</p>
            )}
          </div>
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem onClick={() => router.push('/dashboard/profile')}>
            <IconUserCircle className='mr-2 h-4 w-4' />
            {tMenu('profile')}
          </DropdownMenuItem>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          onClick={handleLogout}
          className='text-destructive focus:text-destructive'
        >
          <IconLogout className='mr-2 h-4 w-4' />
          {tMenu('signOut')}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
