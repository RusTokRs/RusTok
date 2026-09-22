'use client';
import { Avatar, AvatarFallback } from '@/shared/ui/shadcn/avatar';
import type { User } from '../model';

interface UserAvatarProps {
  user: Pick<User, 'name' | 'email'>;
  className?: string;
}

export function UserAvatar({ user, className }: UserAvatarProps) {
  const displayName = user.name || user.email || '';
  const initials = displayName.slice(0, 2).toUpperCase() || 'U';

  return (
    <Avatar className={className}>
      <AvatarFallback className='rounded-lg'>{initials}</AvatarFallback>
    </Avatar>
  );
}
