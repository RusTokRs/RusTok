'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { FormInput } from '@/shared/ui/forms';
import { Form } from '@/shared/ui/shadcn/form';
import { useLocale } from 'next-intl';
import { useForm } from 'react-hook-form';
import { useState } from 'react';
import { toast } from 'sonner';
import { createForumReply } from '../api/forum';
import type { GqlOpts } from '../api/posts';
import { RtJsonEditor } from './rt-json-editor';
import {
  normalizeRtJsonPayload,
  stringifyRtDoc,
  type RtDoc
} from './rt-json-format';

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
  const [doc, setDoc] = useState<RtDoc>({ type: 'doc', content: [] });

  async function submit(values: { locale: string }) {
    const contentJson = normalizeRtJsonPayload(doc, values.locale);
    const plain = stringifyRtDoc(doc, values.locale);
    try {
      await createForumReply(
        topicId,
        {
          locale: values.locale,
          content: plain,
          contentFormat: 'rt_json_v1',
          contentJson
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
          <RtJsonEditor label='Reply content' value={doc} onChange={setDoc} />
          <pre className='bg-muted max-h-44 overflow-auto rounded-md border p-3 text-xs'>
            {stringifyRtDoc(doc, form.watch('locale') || hostLocale)}
          </pre>
          <Button type='submit'>Send reply</Button>
        </CardContent>
      </Form>
    </Card>
  );
}
