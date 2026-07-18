'use client';

import { useTheme } from 'next-themes';
import { Toaster as Sonner, ToasterProps } from 'sonner';

import { cn } from '@/shared/lib/utils';

type RusTokToasterProps = Omit<ToasterProps, 'style'>;

const Toaster = ({ className, ...props }: RusTokToasterProps) => {
  const { theme = 'system' } = useTheme();

  return (
    <Sonner
      theme={theme as ToasterProps['theme']}
      className={cn(
        'toaster group [--normal-bg:var(--popover)] [--normal-text:var(--popover-foreground)] [--normal-border:var(--border)]',
        className
      )}
      {...props}
    />
  );
};

export { Toaster };
