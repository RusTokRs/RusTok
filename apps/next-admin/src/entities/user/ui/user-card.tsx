'use client';
import { Badge } from '@/shared/ui/shadcn/badge';
import type { User } from '../model';
import { UserAvatar } from './user-avatar';

interface UserCardProps {
  user: User;
  className?: string;
}

export function UserCard({ user, className }: UserCardProps) {
  return (
    <div
      className={`flex items-center gap-3 rounded-lg border p-3 ${className ?? ''}`}
    >
      <UserAvatar user={user} />
      <div className='min-w-0 flex-1'>
        <p className='truncate text-sm font-medium'>
          {user.name || user.email}
        </p>
        <p className='text-muted-foreground truncate text-xs'>{user.email}</p>
      </div>
      <Badge variant={user.status === 'ACTIVE' ? 'default' : 'secondary'}>
        {user.status}
      </Badge>
    </div>
  );
}
