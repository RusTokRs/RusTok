import type React from 'react';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Card,
  CardHeader,
  CardDescription,
  CardTitle,
  CardFooter
} from '@/shared/ui/shadcn/card';

export interface StatCardProps {
  title: string;
  value: string;
  change?: number | undefined;
  helper?: string;
  icon?: React.ReactNode;
}

export function formatChange(value: number | undefined): string {
  const next = Number.isFinite(value) ? (value ?? 0) : 0;
  return `${next >= 0 ? '+' : ''}${next.toFixed(1)}%`;
}

export function StatCard({
  title,
  value,
  change,
  helper,
  icon
}: StatCardProps) {
  return (
    <Card className='from-primary/5 to-card @container/card bg-gradient-to-t shadow-xs'>
      <CardHeader>
        <CardDescription>{title}</CardDescription>
        <CardTitle className='text-2xl font-semibold tabular-nums @[250px]/card:text-3xl'>
          {value}
        </CardTitle>
        {icon && (
          <div className='bg-muted text-muted-foreground col-start-2 row-span-2 row-start-1 self-start justify-self-end rounded-lg p-3'>
            {icon}
          </div>
        )}
      </CardHeader>
      {helper && (
        <CardFooter className='flex-col items-start gap-1.5 text-sm'>
          <div className='line-clamp-1 flex items-center gap-2 font-medium'>
            {change !== undefined && (
              <Badge variant='outline'>{formatChange(change)}</Badge>
            )}
            <span>{helper}</span>
          </div>
        </CardFooter>
      )}
    </Card>
  );
}

