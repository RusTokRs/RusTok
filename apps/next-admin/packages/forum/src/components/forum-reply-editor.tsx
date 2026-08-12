'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { FormInput } from '@/shared/ui/forms';
import { RichTextEditor } from '@/shared/ui/rich-text-editor';
import { Form } from '@/shared/ui/shadcn/form';
import {
  emptyRichTextDocument,
  getRichTextProfile,
  richTextDocumentHasText,
  validateRichTextDocument,
  type RichTextDocument
} from '@rustok/richtext';
import { useLocale } from 'next-intl';
import { useForm } from 'react-hook-form';
import { useState } from 'react';
import { toast } from 'sonner';
import { createForumReply, type GqlOpts } from '../api/forum';

export function ForumReplyEditor({
  topicId,
  gqlOpts = {}
}: {
  topicId: string;
  gqlOpts?: GqlOpts;
}) {
  const hostLocale = useLocale();
  const form = useForm<{ locale: string }>({
    defaultValues: { locale: hostLocale }
  });
  const contentLocale = form.watch('locale');
  const [doc, setDoc] = useState<RichTextDocument>(emptyRichTextDocument());

  async function submit(values: { locale: string }) {
    const validation = validateRichTextDocument(
      doc,
      getRichTextProfile('discussion')
    );
    if (!validation.valid || !richTextDocumentHasText(doc)) {
      toast.error(validation.error ?? 'Reply content is required.');
      return;
    }
    try {
      await createForumReply(
        topicId,
        {
          locale: values.locale,
          content: doc
        },
        gqlOpts
      );
      toast.success('Reply posted');
    } catch {
      toast.error('Failed to post reply');
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Forum reply composer</CardTitle>
      </CardHeader>
      <Form form={form} onSubmit={form.handleSubmit(submit)}>
        <CardContent className='space-y-4'>
          <FormInput control={form.control} name='locale' label='Locale' />
          <RichTextEditor
            label='Reply content'
            profile='discussion'
            value={doc}
            contentLocale={contentLocale}
            disabled={form.formState.isSubmitting}
            onChange={setDoc}
          />
          <Button type='submit' disabled={form.formState.isSubmitting}>
            Send reply
          </Button>
        </CardContent>
      </Form>
    </Card>
  );
}
