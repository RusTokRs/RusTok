'use client';

import { RichTextEditor as SharedRichTextEditor } from '@rustok/richtext/react';
import type { RichTextDocument, RichTextProfileId } from '@rustok/richtext';
import { useTranslations } from 'next-intl';
import { useMemo } from 'react';

export function RichTextEditor({
  label,
  value,
  profile,
  contentLocale,
  disabled,
  onChange
}: {
  label: string;
  value: RichTextDocument;
  profile: RichTextProfileId;
  contentLocale: string;
  disabled?: boolean;
  onChange: (doc: RichTextDocument) => void;
}) {
  const t = useTranslations('richText');
  const messages = useMemo(
    () => ({
      bold: t('bold'),
      italic: t('italic'),
      strike: t('strike'),
      code: t('code'),
      heading: t('heading'),
      bullet_list: t('bullet_list'),
      ordered_list: t('ordered_list'),
      blockquote: t('blockquote'),
      code_block: t('code_block'),
      horizontal_rule: t('horizontal_rule'),
      link: t('link'),
      link_url: t('link_url'),
      apply_link: t('apply_link'),
      remove_link: t('remove_link'),
      clear_formatting: t('clear_formatting'),
      undo: t('undo'),
      redo: t('redo'),
      editor: t('editor')
    }),
    [t]
  );

  return (
    <div className='space-y-2'>
      <label className='text-sm font-medium'>{label}</label>
      <SharedRichTextEditor
        frameUrl='/richtext/frame'
        label={label}
        profile={profile}
        value={value}
        messages={messages}
        contentLocale={contentLocale}
        disabled={disabled}
        onChange={onChange}
        className='h-72 w-full border-0'
      />
    </div>
  );
}
