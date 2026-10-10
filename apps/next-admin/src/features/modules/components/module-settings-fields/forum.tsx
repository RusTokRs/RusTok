'use client';

import { useMemo } from 'react';

import { Input } from '@/shared/ui/shadcn/input';
import { Switch } from '@/shared/ui/shadcn/switch';

interface ForumModuleSettingsFieldsProps {
  settingsText: string;
  onSettingsTextChange: (value: string) => void;
  disabled?: boolean;
}

interface ForumFormValues {
  use_reactions: boolean;
  allow_downvotes: boolean;
  allow_user_content_deletion: boolean;
  pre_moderation_enabled: boolean;
  topics_per_page: number;
  replies_per_page: number;
  min_topic_title_length: number;
  max_topic_title_length: number;
}

/**
 * Forum-owned settings UI. Exposes key Discourse/NodeBB-style configuration
 * for posting limits, engagement modes, and moderation policies.
 */
export function ForumModuleSettingsFields({
  settingsText,
  onSettingsTextChange,
  disabled = false
}: ForumModuleSettingsFieldsProps) {
  const values = useMemo<ForumFormValues>(() => {
    try {
      const parsed = JSON.parse(settingsText || '{}');
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        return {
          use_reactions: false,
          allow_downvotes: true,
          allow_user_content_deletion: false,
          pre_moderation_enabled: false,
          topics_per_page: 20,
          replies_per_page: 20,
          min_topic_title_length: 1,
          max_topic_title_length: 255
        };
      }
      return {
        use_reactions:
          parsed.use_reactions === true || parsed.useReactions === true,
        allow_downvotes: parsed.allow_downvotes !== false,
        allow_user_content_deletion: parsed.allow_user_content_deletion === true,
        pre_moderation_enabled: parsed.pre_moderation_enabled === true,
        topics_per_page:
          typeof parsed.topics_per_page === 'number'
            ? parsed.topics_per_page
            : 20,
        replies_per_page:
          typeof parsed.replies_per_page === 'number'
            ? parsed.replies_per_page
            : 20,
        min_topic_title_length:
          typeof parsed.min_topic_title_length === 'number'
            ? parsed.min_topic_title_length
            : 1,
        max_topic_title_length:
          typeof parsed.max_topic_title_length === 'number'
            ? parsed.max_topic_title_length
            : 255
      };
    } catch {
      return {
        use_reactions: false,
        allow_downvotes: true,
        allow_user_content_deletion: false,
        pre_moderation_enabled: false,
        topics_per_page: 20,
        replies_per_page: 20,
        min_topic_title_length: 1,
        max_topic_title_length: 255
      };
    }
  }, [settingsText]);

  const updateFields = (updates: Partial<ForumFormValues>) => {
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
    <div className='space-y-4'>
      <div className='grid grid-cols-2 gap-3'>
        <div className='space-y-1.5'>
          <label className='text-xs font-medium'>Topics per page</label>
          <Input
            type='number'
            min={5}
            max={100}
            value={values.topics_per_page}
            disabled={disabled}
            onChange={(e) =>
              updateFields({ topics_per_page: Number(e.target.value) || 20 })
            }
            className='h-8 text-xs'
          />
        </div>
        <div className='space-y-1.5'>
          <label className='text-xs font-medium'>Replies per page</label>
          <Input
            type='number'
            min={5}
            max={100}
            value={values.replies_per_page}
            disabled={disabled}
            onChange={(e) =>
              updateFields({ replies_per_page: Number(e.target.value) || 20 })
            }
            className='h-8 text-xs'
          />
        </div>
      </div>

      <div className='grid grid-cols-2 gap-3'>
        <div className='space-y-1.5'>
          <label className='text-xs font-medium'>Min title length</label>
          <Input
            type='number'
            min={1}
            max={50}
            value={values.min_topic_title_length}
            disabled={disabled}
            onChange={(e) =>
              updateFields({
                min_topic_title_length: Number(e.target.value) || 1
              })
            }
            className='h-8 text-xs'
          />
        </div>
        <div className='space-y-1.5'>
          <label className='text-xs font-medium'>
            Max title length (0 = no limit)
          </label>
          <Input
            type='number'
            min={0}
            value={values.max_topic_title_length}
            disabled={disabled}
            onChange={(e) =>
              updateFields({
                max_topic_title_length:
                  e.target.value === ''
                    ? 255
                    : Math.max(0, Number(e.target.value))
              })
            }
            className='h-8 text-xs'
          />
        </div>
      </div>

      <div className='space-y-2 pt-1'>
        <div className='bg-muted/20 flex items-center justify-between rounded-lg border p-3'>
          <div className='space-y-0.5 pr-4'>
            <label className='text-xs font-medium'>Use Shared Reactions</label>
            <p className='text-muted-foreground text-[11px]'>
              Switch from internal voting to rich emoji reactions via the
              Reactions module.
            </p>
          </div>
          <Switch
            checked={values.use_reactions}
            disabled={disabled}
            onCheckedChange={(checked) =>
              updateFields({ use_reactions: checked })
            }
          />
        </div>

        <div className='bg-muted/20 flex items-center justify-between rounded-lg border p-3'>
          <div className='space-y-0.5 pr-4'>
            <label className='text-xs font-medium'>Allow Downvotes</label>
            <p className='text-muted-foreground text-[11px]'>
              Permit negative voting on topics and replies. Turn off for
              like-only mode.
            </p>
          </div>
          <Switch
            checked={values.allow_downvotes}
            disabled={disabled}
            onCheckedChange={(checked) =>
              updateFields({ allow_downvotes: checked })
            }
          />
        </div>

        <div className='bg-muted/20 flex items-center justify-between rounded-lg border p-3'>
          <div className='space-y-0.5 pr-4'>
            <label className='text-xs font-medium'>Allow Author Deletion</label>
            <p className='text-muted-foreground text-[11px]'>
              Let topic and reply authors delete their own content. Moderators
              are not affected.
            </p>
          </div>
          <Switch
            checked={values.allow_user_content_deletion}
            disabled={disabled}
            onCheckedChange={(checked) =>
              updateFields({ allow_user_content_deletion: checked })
            }
          />
        </div>

        <div className='bg-muted/20 flex items-center justify-between rounded-lg border p-3'>
          <div className='space-y-0.5 pr-4'>
            <label className='text-xs font-medium'>Pre-moderation</label>
            <p className='text-muted-foreground text-[11px]'>
              Hold newly submitted topics and replies for moderator review
              before publishing. Held items are visible only to their author
              and to moderators.
            </p>
          </div>
          <Switch
            checked={values.pre_moderation_enabled}
            disabled={disabled}
            onCheckedChange={(checked) =>
              updateFields({ pre_moderation_enabled: checked })
            }
          />
        </div>
      </div>
    </div>
  );
}
