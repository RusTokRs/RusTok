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
import { useForm } from 'react-hook-form';
import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { createForumReply, type GqlOpts } from '../api/forum';

interface ForumReplyEditorProps {
  topicId: string;
  initialContentLocale: string;
  gqlOpts?: GqlOpts;
}

export function ForumReplyEditor(props: ForumReplyEditorProps) {
  const sessionKey = JSON.stringify([
    props.topicId,
    props.initialContentLocale,
    props.gqlOpts?.tenantId,
    props.gqlOpts?.tenantSlug,
    props.gqlOpts?.token
  ]);
  return <ForumReplyEditorSession key={sessionKey} {...props} />;
}

function ForumReplyEditorSession({
  topicId,
  initialContentLocale,
  gqlOpts = {}
}: ForumReplyEditorProps) {
  const form = useForm<{ locale: string }>({
    defaultValues: { locale: initialContentLocale }
  });
  const contentLocale = form.watch('locale');
  const [doc, setDoc] = useState<RichTextDocument>(emptyRichTextDocument());

  const documentRef = useRef(doc);
  const submittingRef = useRef(false);
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  function updateDocument(document: RichTextDocument) {
    documentRef.current = document;
    setDoc(document);
  }

  async function submit(values: { locale: string }) {
    if (submittingRef.current || !mountedRef.current) return;
    const submittedDocument = documentRef.current;
    const validation = validateRichTextDocument(
      submittedDocument,
      getRichTextProfile('discussion')
    );
    if (!validation.valid || !richTextDocumentHasText(submittedDocument)) {
      toast.error(validation.error ?? 'Reply content is required.');
      return;
    }
    const submittedContent = JSON.stringify(submittedDocument);
    submittingRef.current = true;
    try {
      await createForumReply(
        topicId,
        {
          locale: values.locale,
          content: submittedDocument
        },
        gqlOpts
      );
      if (!mountedRef.current) return;
      // Do not clear newer frame input delivered while the request was pending.
      if (JSON.stringify(documentRef.current) === submittedContent) {
        updateDocument(emptyRichTextDocument());
        form.reset({ locale: values.locale });
      }
      toast.success('Reply posted');
    } catch {
      if (mountedRef.current) toast.error('Failed to post reply');
    } finally {
      submittingRef.current = false;
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Forum reply composer</CardTitle>
      </CardHeader>
      <Form form={form} onSubmit={form.handleSubmit(submit)}>
        <CardContent className='space-y-4'>
          <FormInput
            control={form.control}
            name='locale'
            label='Locale'
            dir='ltr'
            disabled={form.formState.isSubmitting}
          />
          <RichTextEditor
            label='Reply content'
            profile='discussion'
            value={doc}
            contentLocale={contentLocale}
            disabled={form.formState.isSubmitting}
            onChange={updateDocument}
          />
          <Button type='submit' disabled={form.formState.isSubmitting}>
            Send reply
          </Button>
        </CardContent>
      </Form>
    </Card>
  );
}
