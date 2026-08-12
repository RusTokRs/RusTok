'use client';

import {
  emptyRichTextDocument,
  richTextDocumentHasText,
  type RichTextDocument
} from '@rustok/richtext';
import { RichTextEditor } from '@rustok/richtext/react';
import { useTranslations } from 'next-intl';
import { useMemo, useState } from 'react';
import type { FormEvent } from 'react';

export function CommentComposer({
  contentLocale,
  canSubmit,
  onSubmit
}: {
  contentLocale: string;
  canSubmit: boolean;
  onSubmit: (document: RichTextDocument) => Promise<void>;
}) {
  const t = useTranslations('Comments.composer');
  const richText = useTranslations('richText');
  const [document, setDocument] = useState<RichTextDocument>(emptyRichTextDocument());
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);
  const messages = useMemo(
    () => ({
      bold: richText('bold'),
      italic: richText('italic'),
      strike: richText('strike'),
      code: richText('code'),
      heading: richText('heading'),
      bullet_list: richText('bullet_list'),
      ordered_list: richText('ordered_list'),
      blockquote: richText('blockquote'),
      code_block: richText('code_block'),
      horizontal_rule: richText('horizontal_rule'),
      link: richText('link'),
      link_url: richText('link_url'),
      apply_link: richText('apply_link'),
      remove_link: richText('remove_link'),
      clear_formatting: richText('clear_formatting'),
      undo: richText('undo'),
      redo: richText('redo'),
      editor: richText('editor')
    }),
    [richText]
  );

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!richTextDocumentHasText(document)) {
      setError(t('emptyError'));
      setSuccess(false);
      return;
    }
    setPending(true);
    setError(null);
    setSuccess(false);
    try {
      await onSubmit(document);
      setDocument(emptyRichTextDocument());
      setSuccess(true);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : t('submitError'));
    } finally {
      setPending(false);
    }
  }

  return (
    <section className="mt-6 rounded-2xl border border-border bg-card/50 p-5">
      <h3 className="text-base font-semibold text-foreground">{t('title')}</h3>
      {!canSubmit ? (
        <p className="mt-3 rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
          {t('signInRequired')}
        </p>
      ) : (
        <form className="mt-4 space-y-3" onSubmit={submit}>
          <label className="block text-sm font-medium">{t('editorLabel')}</label>
          <RichTextEditor
            frameUrl="/richtext/frame"
            label={t('editorLabel')}
            profile="comment"
            value={document}
            messages={messages}
            contentLocale={contentLocale}
            disabled={pending}
            onChange={setDocument}
            onError={(_code, message) => setError(message)}
            className="h-72 w-full border-0"
          />
          <p className="text-xs text-muted-foreground">{t('hint')}</p>
          {error && <p className="text-sm text-destructive" role="alert">{error}</p>}
          {success && <p className="text-sm text-emerald-700 dark:text-emerald-300" role="status">{t('success')}</p>}
          <button
            type="submit"
            disabled={pending}
            className="rounded-full bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground transition hover:opacity-95 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {pending ? t('submitting') : t('submit')}
          </button>
        </form>
      )}
    </section>
  );
}
