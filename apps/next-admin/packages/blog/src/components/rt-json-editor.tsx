'use client';

import Link from '@tiptap/extension-link';
import { EditorContent, useEditor, type Editor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { cn } from '@/shared/lib/utils';
import type { RtDoc } from './rt-json-format';

function ToolbarButton({
  editor,
  label,
  onClick,
  active = false
}: {
  editor: Editor | null;
  label: string;
  onClick: () => void;
  active?: boolean;
}) {
  return (
    <Button
      type='button'
      variant={active ? 'default' : 'outline'}
      size='sm'
      disabled={!editor}
      onClick={onClick}
    >
      {label}
    </Button>
  );
}

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
    extensions: [
      StarterKit.configure({
        heading: {
          levels: [1, 2, 3, 4, 5, 6]
        }
      }),
      Link.configure({
        openOnClick: false,
        autolink: true,
        linkOnPaste: true,
        protocols: ['http', 'https', 'mailto']
      })
    ],
    content: value,
    editorProps: {
      attributes: {
        class:
          'min-h-56 rounded-b-md border border-t-0 border-input bg-background px-3 py-3 text-sm focus-visible:outline-none'
      }
    },
    onUpdate: ({ editor: instance }) => {
      onChange(instance.getJSON() as RtDoc);
    }
  });

  useEffect(() => {
    if (!editor) {
      return;
    }

    const current = JSON.stringify(editor.getJSON());
    const next = JSON.stringify(value);
    if (current !== next) {
      editor.commands.setContent(value, { emitUpdate: false });
    }
  }, [editor, value]);

  function setLink() {
    if (!editor) {
      return;
    }

    const previousUrl = editor.getAttributes('link').href as string | undefined;
    const url = window.prompt('Enter link URL', previousUrl ?? 'https://');

    if (url === null) {
      return;
    }

    const trimmed = url.trim();
    if (!trimmed) {
      editor.chain().focus().extendMarkRange('link').unsetLink().run();
      return;
    }

    editor
      .chain()
      .focus()
      .extendMarkRange('link')
      .setLink({ href: trimmed })
      .run();
  }

  return (
    <div className='space-y-2'>
      <Label>{label}</Label>
      <div className='border-input rounded-md border'>
        <div className='border-input flex flex-wrap gap-2 border-b p-2'>
          <ToolbarButton
            editor={editor}
            label='Bold'
            active={editor?.isActive('bold')}
            onClick={() => editor?.chain().focus().toggleBold().run()}
          />
          <ToolbarButton
            editor={editor}
            label='Italic'
            active={editor?.isActive('italic')}
            onClick={() => editor?.chain().focus().toggleItalic().run()}
          />
          <ToolbarButton
            editor={editor}
            label='Strike'
            active={editor?.isActive('strike')}
            onClick={() => editor?.chain().focus().toggleStrike().run()}
          />
          <ToolbarButton
            editor={editor}
            label='Code'
            active={editor?.isActive('code')}
            onClick={() => editor?.chain().focus().toggleCode().run()}
          />
          <ToolbarButton
            editor={editor}
            label='H1'
            active={editor?.isActive('heading', { level: 1 })}
            onClick={() =>
              editor?.chain().focus().toggleHeading({ level: 1 }).run()
            }
          />
          <ToolbarButton
            editor={editor}
            label='H2'
            active={editor?.isActive('heading', { level: 2 })}
            onClick={() =>
              editor?.chain().focus().toggleHeading({ level: 2 }).run()
            }
          />
          <ToolbarButton
            editor={editor}
            label='Bullet'
            active={editor?.isActive('bulletList')}
            onClick={() => editor?.chain().focus().toggleBulletList().run()}
          />
          <ToolbarButton
            editor={editor}
            label='Numbered'
            active={editor?.isActive('orderedList')}
            onClick={() => editor?.chain().focus().toggleOrderedList().run()}
          />
          <ToolbarButton
            editor={editor}
            label='Quote'
            active={editor?.isActive('blockquote')}
            onClick={() => editor?.chain().focus().toggleBlockquote().run()}
          />
          <ToolbarButton
            editor={editor}
            label='Code block'
            active={editor?.isActive('codeBlock')}
            onClick={() => editor?.chain().focus().toggleCodeBlock().run()}
          />
          <ToolbarButton
            editor={editor}
            label='Rule'
            onClick={() => editor?.chain().focus().setHorizontalRule().run()}
          />
          <ToolbarButton
            editor={editor}
            label='Link'
            active={editor?.isActive('link')}
            onClick={setLink}
          />
          <ToolbarButton
            editor={editor}
            label='Clear'
            onClick={() =>
              editor?.chain().focus().unsetAllMarks().clearNodes().run()
            }
          />
        </div>
        <EditorContent
          editor={editor}
          className={cn(
            'prose prose-sm max-w-none',
            '[&_pre]:bg-muted [&_ol]:list-decimal [&_ol]:pl-6 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:p-3 [&_ul]:list-disc [&_ul]:pl-6'
          )}
        />
      </div>
      <p className='text-muted-foreground text-xs'>
        Editor stores content as canonical `rt_json_v1` payload after submit.
      </p>
    </div>
  );
}
