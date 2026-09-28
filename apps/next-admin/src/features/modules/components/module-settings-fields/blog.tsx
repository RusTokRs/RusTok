'use client';

import { useMemo } from 'react';

import { Switch } from '@/shared/ui/shadcn/switch';

interface BlogModuleSettingsFieldsProps {
  settingsText: string;
  onSettingsTextChange: (value: string) => void;
  disabled?: boolean;
}

type CommentsMode = 'disabled' | 'read_only' | 'open';

/**
 * Blog-owned settings UI. Exposes public comment surface policy and Reactions integration.
 */
export function BlogModuleSettingsFields({
  settingsText,
  onSettingsTextChange,
  disabled = false
}: BlogModuleSettingsFieldsProps) {
  const { comments_mode, use_reactions } = useMemo(() => {
    try {
      const parsed = JSON.parse(settingsText || '{}');
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        return { comments_mode: 'open' as CommentsMode, use_reactions: false };
      }
      const rawMode = parsed.comments_mode;
      const validMode: CommentsMode =
        rawMode === 'disabled' || rawMode === 'read_only' || rawMode === 'open'
          ? rawMode
          : 'open';
      return {
        comments_mode: validMode,
        use_reactions: parsed.use_reactions === true
      };
    } catch {
      return { comments_mode: 'open' as CommentsMode, use_reactions: false };
    }
  }, [settingsText]);

  const updateSettings = (
    updates: Partial<{ comments_mode: CommentsMode; use_reactions: boolean }>
  ) => {
    try {
      const parsed = JSON.parse(settingsText || '{}');
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        return;
      }
      onSettingsTextChange(JSON.stringify({ ...parsed, ...updates }, null, 2));
    } catch {
      // The generic JSON editor remains the authority while the document is invalid.
    }
  };

  return (
    <div className='space-y-3'>
      <div className='bg-muted/20 rounded-lg border p-3'>
        <div className='mb-2 space-y-1'>
          <label className='text-sm font-medium'>Comments Policy</label>
          <p className='text-muted-foreground text-xs'>
            Controls the public comment surface on Blog posts without mutating
            the Comments module lifecycle.
          </p>
        </div>
        <select
          value={comments_mode}
          disabled={disabled}
          onChange={(e) =>
            updateSettings({ comments_mode: e.target.value as CommentsMode })
          }
          className='border-input bg-background ring-offset-background focus-visible:ring-ring w-full rounded-md border px-3 py-1.5 text-xs focus-visible:ring-2 focus-visible:outline-none'
        >
          <option value='open'>Open (normal reading and commenting)</option>
          <option value='read_only'>
            Read-only (existing comments shown, new comments closed)
          </option>
          <option value='disabled'>
            Disabled (comment surface completely hidden)
          </option>
        </select>
      </div>

      <div className='bg-muted/20 flex items-center justify-between rounded-lg border p-3'>
        <div className='space-y-1 pr-4'>
          <label className='text-sm font-medium'>Use Shared Reactions</label>
          <p className='text-muted-foreground text-xs'>
            Enables Blog post reactions powered by the shared Reactions module.
          </p>
        </div>
        <Switch
          checked={use_reactions}
          disabled={disabled}
          onCheckedChange={(checked) =>
            updateSettings({ use_reactions: checked })
          }
        />
      </div>
    </div>
  );
}
