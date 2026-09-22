import * as React from 'react';
import {
  Avatar as ShadcnAvatar,
  AvatarFallback,
  AvatarImage
} from '@/components/ui/avatar';
import { cn } from '@/lib/utils';

export interface AvatarProps {
  src?: string;
  alt?: string;
  fallback?: string;
  className?: string;
}

export function Avatar({ src, alt, fallback, className }: AvatarProps) {
  const initials = fallback
    ? fallback
        .split(' ')
        .map((word) => word[0])
        .join('')
        .toUpperCase()
        .slice(0, 2)
    : '?';

  return (
    <ShadcnAvatar className={cn(className)}>
      {src && <AvatarImage src={src} alt={alt} />}
      <AvatarFallback>{initials}</AvatarFallback>
    </ShadcnAvatar>
  );
}
