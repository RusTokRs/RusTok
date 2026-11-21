'use client';

import { useId, useMemo, useState } from 'react';
import { useTranslations } from '@rustok/next-fluent/client';
import { isLosslessNumber } from 'lossless-json';
import {
  mergeSettingsJson,
  parseSettingsObject,
  parseSettingsUnsignedInteger
} from '../module-settings-json';

import { Input } from '@/shared/ui/shadcn/input';
import { Switch } from '@/shared/ui/shadcn/switch';

interface ForumModuleSettingsFieldsProps {
  settingsText: string;
  onSettingsTextChange: (value: string) => void;
  onFieldValidityChange: (field: string, valid: boolean) => void;
  disabled?: boolean;
}

type NumericField =
  | 'topics_per_page'
  | 'replies_per_page'
  | 'min_topic_title_length'
  | 'max_topic_title_length';

/** A draft may be invalid while typing, but can never be silently submitted. */
function ForumNumberField({
  field,
  label,
  value,
  disabled,
  onFieldValidityChange,
  onValueChange
}: {
  field: NumericField;
  label: string;
  value: string;
  disabled: boolean;
  onFieldValidityChange: (field: string, valid: boolean) => void;
  onValueChange: (field: NumericField, value: unknown) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const id = useId();
  const t = useTranslations('modules');
  return (
    <div className='space-y-1.5'>
      <label htmlFor={id} className='text-xs font-medium'>
        {label}
      </label>
      <Input
        id={id}
        type='text'
        inputMode='numeric'
        value={draft ?? value}
        disabled={disabled}
        aria-invalid={draft !== null}
        aria-describedby={draft !== null ? `${id}-error` : undefined}
        onChange={(event) => {
          const text = event.target.value;
          try {
            const number = parseSettingsUnsignedInteger(text);
            onValueChange(field, number);
            setDraft(null);
            onFieldValidityChange(field, true);
          } catch {
            setDraft(text);
            onFieldValidityChange(field, false);
          }
        }}
        className='h-8 text-xs'
      />
      {draft !== null && (
        <p id={`${id}-error`} className='text-destructive text-xs'>
          {t('settings.integer.invalid')}
        </p>
      )}
    </div>
  );
}

/** Forum settings controls never coerce JSON numbers through JavaScript number. */
export function ForumModuleSettingsFields({
  settingsText,
  onSettingsTextChange,
  onFieldValidityChange,
  disabled = false
}: ForumModuleSettingsFieldsProps) {
  const settings = useMemo(() => {
    try {
      return parseSettingsObject(settingsText);
    } catch {
      return null;
    }
  }, [settingsText]);
  const fieldDisabled = disabled || settings === null;
  const integerText = (field: NumericField, fallback: string) => {
    const value = settings?.[field];
    return value === undefined
      ? fallback
      : isLosslessNumber(value)
        ? value.toString()
        : '';
  };
  const updateFields = (updates: Record<string, unknown>) => {
    onSettingsTextChange(mergeSettingsJson(settingsText, updates));
  };
  const values = {
    use_reactions:
      settings?.use_reactions === true || settings?.useReactions === true,
    allow_downvotes: settings?.allow_downvotes !== false,
    allow_user_content_deletion: settings?.allow_user_content_deletion === true,
    pre_moderation_enabled: settings?.pre_moderation_enabled === true
  };
  const numberField = (
    field: NumericField,
    label: string,
    fallback: string
  ) => (
    <ForumNumberField
      field={field}
      label={label}
      value={integerText(field, fallback)}
      disabled={fieldDisabled}
      onFieldValidityChange={onFieldValidityChange}
      onValueChange={(field, value) => updateFields({ [field]: value })}
    />
  );

  return (
    <div className='space-y-4'>
      <div className='grid grid-cols-2 gap-3'>
        {numberField('topics_per_page', 'Topics per page', '20')}
        {numberField('replies_per_page', 'Replies per page', '20')}
      </div>
      <div className='grid grid-cols-2 gap-3'>
        {numberField('min_topic_title_length', 'Min title length', '1')}
        {numberField(
          'max_topic_title_length',
          'Max title length (0 = no limit)',
          '255'
        )}
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
            disabled={fieldDisabled}
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
            disabled={fieldDisabled}
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
            disabled={fieldDisabled}
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
              before publishing. Held items are visible only to their author and
              to moderators.
            </p>
          </div>
          <Switch
            checked={values.pre_moderation_enabled}
            disabled={fieldDisabled}
            onCheckedChange={(checked) =>
              updateFields({ pre_moderation_enabled: checked })
            }
          />
        </div>
      </div>
    </div>
  );
}
