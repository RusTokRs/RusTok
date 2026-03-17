'use client';

import { Label } from '@/components/ui/label';
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import type { RtDoc } from './rt-json-format';

export function RtJsonEditor({
  label,
  value,
  onChange
}: {
  label: string;
  value: RtDoc;
  onChange: (doc: RtDoc) => void;
}) {
  const editor = useEditor({
    immediatelyRender: false,
    extensions: [StarterKit],
    content: value,
    onUpdate: ({ editor: instance }) => {
      onChange(instance.getJSON() as RtDoc);
    },
    editorProps: {
      attributes: {
        class: 'min-h-56 rounded-md border border-input bg-background p-3 text-sm focus-visible:outline-none'
      }
    }
  });

  return (
    <div className='space-y-2'>
      <Label>{label}</Label>
      <EditorContent editor={editor} />
    </div>
  );
}
