'use client';

import { useMemo } from 'react';

import { Switch } from '@/shared/ui/shadcn/switch';

interface ForumModuleSettingsFieldsProps {
  settingsText: string;
  onSettingsTextChange: (value: string) => void;
  disabled?: boolean;
}

/**
 * Forum-owned settings UI. The generic module settings dialog only owns the
 * JSON editor/CAS lifecycle; module-specific fields stay outside that generic
 * control so new modules do not create slug-specific branches there.
 */
export function ForumModuleSettingsFields({
  settingsText,
  onSettingsTextChange,
  disabled = false
}: ForumModuleSettingsFieldsProps) {
  const useReactions = useMemo(() => {
    try {
      const parsed = JSON.parse(settingsText || '{}');
      return Boolean(
        parsed && typeof parsed === 'object' && !Array.isArray(parsed) && parsed.useReactions === true
      );
    } catch {
      return false;
    }
  }, [settingsText]);

  const handleChange = (checked: boolean) => {
    try {
      const parsed = JSON.parse(settingsText || '{}');
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        return;
      }
      onSettingsTextChange(
        JSON.stringify({ ...parsed, useReactions: checked }, null, 2)
      );
    } catch {
      // The generic JSON editor remains the authority while the document is invalid.
    }
  };

  return (
    <div className='flex items-center justify-between rounded-lg border bg-muted/20 p-3'>
      <div className='space-y-1 pr-4'>
        <label className='text-sm font-medium'>
          Use Reactions instead of internal voting
        </label>
        <p className='text-muted-foreground text-xs'>
          Forum-specific setting. The shared Reactions module can stay enabled for other modules.
        </p>
      </div>
      <Switch checked={useReactions} disabled={disabled} onCheckedChange={handleChange} />
    </div>
  );
}
