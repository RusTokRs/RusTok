import { Avatar, AvatarFallback } from '@/shared/ui/shadcn/avatar';

interface UserAvatarProfileProps {
  className?: string;
  showInfo?: boolean;
  user: { email: string; name: string | null; role?: string } | null;
}

export function UserAvatarProfile({
  className,
  showInfo = false,
  user
}: UserAvatarProfileProps) {
  const displayName = user?.name || user?.email || '';
  const initials = displayName.slice(0, 2).toUpperCase() || 'U';

  return (
    <div className='flex items-center gap-2'>
      <Avatar className={className}>
        <AvatarFallback className='rounded-lg'>{initials}</AvatarFallback>
      </Avatar>
      {showInfo && (
        <div className='grid flex-1 text-left text-sm leading-tight'>
          <span className='truncate font-semibold'>
            {user?.name || user?.email || ''}
          </span>
          <span className='truncate text-xs'>{user?.email || ''}</span>
        </div>
      )}
    </div>
  );
}
