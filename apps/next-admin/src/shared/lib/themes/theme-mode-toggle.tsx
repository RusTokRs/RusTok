'use client';

import { IconBrightness } from '@tabler/icons-react';
import { useTheme } from 'next-themes';
import * as React from 'react';

import { Button } from '@/shared/ui/shadcn/button';

export function ThemeModeToggle() {
  const { setTheme, resolvedTheme } = useTheme();

  const handleThemeToggle = React.useCallback(() => {
    const newMode = resolvedTheme === 'dark' ? 'light' : 'dark';
    setTheme(newMode);
  }, [resolvedTheme, setTheme]);

  return (
    <Button
      variant='secondary'
      size='icon'
      className='group/toggle size-8'
      onClick={handleThemeToggle}
    >
      <IconBrightness />
      <span className='sr-only'>Toggle theme</span>
    </Button>
  );
}
