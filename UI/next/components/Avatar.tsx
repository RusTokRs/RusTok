import * as React from 'react';
import {
  Avatar as ShadcnAvatar,
  AvatarFallback,
  AvatarImage
} from '@/components/ui/avatar';
import { cn } from '@/lib/utils';

export type IUAvatarSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl';

const sizeClasses: Record<IUAvatarSize, string> = {
  xs: 'h-6 w-6 text-[10px]',
  sm: 'h-8 w-8 text-xs',
  md: 'h-10 w-10 text-sm',
  lg: 'h-12 w-12 text-base',
  xl: 'h-16 w-16 text-lg'
};

export interface AvatarProps {
  src?: string;
  alt?: string;
  fallback?: string;
  size?: IUAvatarSize;
  className?: string;
}

export function Avatar({ src, alt, fallback, size = 'md', className }: AvatarProps) {
  const initials = fallback
    ? fallback
        .split(' ')
        .map((word) => word[0])
        .join('')
        .toUpperCase()
        .slice(0, 2)
    : '?';

  return (
    <ShadcnAvatar className={cn(sizeClasses[size], className)}>
      {src && <AvatarImage src={src} alt={alt} />}
      <AvatarFallback>{initials}</AvatarFallback>
    </ShadcnAvatar>
  );
}
