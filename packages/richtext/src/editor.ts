import { Editor, type Extensions } from '@tiptap/core';
import Blockquote from '@tiptap/extension-blockquote';
import Bold from '@tiptap/extension-bold';
import BulletList from '@tiptap/extension-bullet-list';
import Code from '@tiptap/extension-code';
import CodeBlock from '@tiptap/extension-code-block';
import Document from '@tiptap/extension-document';
import HardBreak from '@tiptap/extension-hard-break';
import Heading from '@tiptap/extension-heading';
import HorizontalRule from '@tiptap/extension-horizontal-rule';
import Italic from '@tiptap/extension-italic';
import Link from '@tiptap/extension-link';
import ListItem from '@tiptap/extension-list-item';
import OrderedList from '@tiptap/extension-ordered-list';
import Paragraph from '@tiptap/extension-paragraph';
import Strike from '@tiptap/extension-strike';
import Text from '@tiptap/extension-text';
import { UndoRedo } from '@tiptap/extensions';
import type {
  RichTextDocument,
  RichTextProfileManifest
} from './generated/contracts';

export interface CreateRichTextEditorOptions {
  element: HTMLElement;
  profile: RichTextProfileManifest;
  document: RichTextDocument;
  editable: boolean;
  onChange(document: RichTextDocument): void;
  onFocusChange(focused: boolean): void;
}

export function createRichTextEditor(
  options: CreateRichTextEditorOptions
): Editor {
  return new Editor({
    element: options.element,
    extensions: extensionsForProfile(options.profile),
    content: options.document,
    editable: options.editable,
    injectCSS: false,
    editorProps: {
      attributes: {
        class: 'richtext-editor__content',
        role: 'textbox',
        'aria-multiline': 'true'
      }
    },
    onUpdate: ({ editor }) => {
      options.onChange(editor.getJSON() as RichTextDocument);
    },
    onFocus: () => options.onFocusChange(true),
    onBlur: () => options.onFocusChange(false)
  });
}

export function extensionsForProfile(
  profile: RichTextProfileManifest
): Extensions {
  const extensions: Extensions = [Document, Paragraph, Text, UndoRedo];
  if (profile.marks.includes('bold')) extensions.push(Bold);
  if (profile.marks.includes('italic')) extensions.push(Italic);
  if (profile.marks.includes('strike')) extensions.push(Strike);
  if (profile.marks.includes('code')) extensions.push(Code);
  if (profile.marks.includes('link')) {
    extensions.push(
      Link.configure({
        openOnClick: false,
        autolink: true,
        linkOnPaste: true,
        protocols: ['http', 'https', 'mailto'],
        HTMLAttributes: { target: null, rel: null, class: null }
      })
    );
  }
  if (profile.nodes.includes('heading')) {
    extensions.push(
      Heading.configure({
        levels: profile.heading_levels as Array<1 | 2 | 3 | 4 | 5 | 6>
      })
    );
  }
  if (profile.nodes.includes('bulletList')) extensions.push(BulletList);
  if (profile.nodes.includes('orderedList')) extensions.push(OrderedList);
  if (profile.nodes.includes('listItem')) extensions.push(ListItem);
  if (profile.nodes.includes('blockquote')) extensions.push(Blockquote);
  if (profile.nodes.includes('codeBlock')) extensions.push(CodeBlock);
  if (profile.nodes.includes('horizontalRule')) extensions.push(HorizontalRule);
  if (profile.nodes.includes('hardBreak')) extensions.push(HardBreak);
  return extensions;
}

export function setEditorDocument(
  editor: Editor,
  document: RichTextDocument
): void {
  if (JSON.stringify(editor.getJSON()) === JSON.stringify(document)) return;
  editor.commands.setContent(document, { emitUpdate: false });
}
