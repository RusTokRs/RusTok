'use client';

import * as React from 'react';
import { Info } from 'lucide-react';
import { Button } from '@/shared/ui/shadcn/button';
import { useInfobar, type InfobarContent } from '@/shared/ui/shadcn/infobar';
import { cn } from '@/shared/lib/utils';

interface InfoButtonProps
  extends Omit<React.ComponentProps<typeof Button>, 'content'> {
  content: InfobarContent;
  variant?:
    | 'default'
    | 'ghost'
    | 'outline'
    | 'secondary'
    | 'destructive'
    | 'link';
  size?: 'default' | 'sm' | 'lg' | 'icon';
}

export function InfoButton({
  content,
  className,
  variant = 'ghost',
  size = 'icon',
  ...props
}: InfoButtonProps) {
  const { setContent, setOpen } = useInfobar();

  const handleClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    setContent(content);
    setOpen(true);
    props.onClick?.(e);
  };

  return (
    <Button
      variant={variant}
      size={size}
      className={cn('shrink-0', className)}
      onClick={handleClick}
      aria-label='Show information'
      {...props}
    >
      <Info className='h-4 w-4' />
      <span className='sr-only'>Show information</span>
    </Button>
  );
}
