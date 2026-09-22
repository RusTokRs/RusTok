import * as React from 'react';
import { Skeleton as ShadcnSkeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';

export interface SkeletonProps extends React.ComponentProps<'div'> {
  className?: string;
}

export function Skeleton({ className, ...props }: SkeletonProps) {
  return <ShadcnSkeleton className={cn(className)} {...props} />;
}
