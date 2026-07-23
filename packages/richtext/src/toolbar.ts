import type { Editor } from '@tiptap/core';
import type { RichTextProfileManifest } from './generated/contracts';
import type { RichTextMessages } from './messages';

interface ToolbarAction {
  key: keyof RichTextMessages;
  active?: () => boolean;
  enabled?: () => boolean;
  run(): void;
}

export function mountToolbar(
  container: HTMLElement,
  editor: Editor,
  profile: RichTextProfileManifest,
  messages: RichTextMessages
): () => void {
  const actions: ToolbarAction[] = [];
  const add = (action: ToolbarAction) => actions.push(action);
  if (profile.marks.includes('bold')) add(toggle('bold', 'bold', () => editor.chain().focus().toggleBold().run()));
  if (profile.marks.includes('italic')) add(toggle('italic', 'italic', () => editor.chain().focus().toggleItalic().run()));
  if (profile.marks.includes('strike')) add(toggle('strike', 'strike', () => editor.chain().focus().toggleStrike().run()));
  if (profile.marks.includes('code')) add(toggle('code', 'code', () => editor.chain().focus().toggleCode().run()));
  for (const level of profile.heading_levels) {
    const headingLevel = level as 1 | 2 | 3 | 4 | 5 | 6;
    add({
      key: 'heading',
      active: () => editor.isActive('heading', { level: headingLevel }),
      run: () => editor.chain().focus().toggleHeading({ level: headingLevel }).run()
    });
  }
  if (profile.nodes.includes('bulletList')) add(toggle('bullet_list', 'bulletList', () => editor.chain().focus().toggleBulletList().run()));
  if (profile.nodes.includes('orderedList')) add(toggle('ordered_list', 'orderedList', () => editor.chain().focus().toggleOrderedList().run()));
  if (profile.nodes.includes('blockquote')) add(toggle('blockquote', 'blockquote', () => editor.chain().focus().toggleBlockquote().run()));
  if (profile.nodes.includes('codeBlock')) add(toggle('code_block', 'codeBlock', () => editor.chain().focus().toggleCodeBlock().run()));
  if (profile.nodes.includes('horizontalRule')) add({ key: 'horizontal_rule', run: () => editor.chain().focus().setHorizontalRule().run() });

  const buttons: HTMLButtonElement[] = [];
  actions.forEach((action, index) => {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'richtext-toolbar__button';
    const label = action.key === 'heading'
      ? `${messages.heading} ${profile.heading_levels[index - actions.findIndex((item) => item.key === 'heading')] ?? ''}`.trim()
      : messages[action.key];
    button.textContent = label;
    button.setAttribute('aria-label', label);
    button.addEventListener('click', action.run);
    container.append(button);
    buttons.push(button);
  });

  if (profile.marks.includes('link')) mountLinkControls(container, editor, messages);
  const clear = toolbarButton(messages.clear_formatting, () => editor.chain().focus().unsetAllMarks().clearNodes().run());
  const undo = toolbarButton(messages.undo, () => editor.chain().focus().undo().run());
  const redo = toolbarButton(messages.redo, () => editor.chain().focus().redo().run());
  container.append(clear, undo, redo);

  const refresh = () => {
    actions.forEach((action, index) => {
      buttons[index].setAttribute('aria-pressed', String(action.active?.() ?? false));
      buttons[index].disabled = action.enabled ? !action.enabled() : !editor.isEditable;
    });
    clear.disabled = !editor.isEditable;
    undo.disabled = !editor.isEditable || !editor.can().undo();
    redo.disabled = !editor.isEditable || !editor.can().redo();
  };
  editor.on('selectionUpdate', refresh);
  editor.on('transaction', refresh);
  refresh();
  return () => {
    editor.off('selectionUpdate', refresh);
    editor.off('transaction', refresh);
    container.replaceChildren();
  };

  function toggle(
    key: keyof RichTextMessages,
    name: string,
    run: () => void
  ): ToolbarAction {
    return { key, active: () => editor.isActive(name), run };
  }
}

function mountLinkControls(
  container: HTMLElement,
  editor: Editor,
  messages: RichTextMessages
): void {
  const details = document.createElement('details');
  details.className = 'richtext-toolbar__link';
  const summary = document.createElement('summary');
  summary.className = 'richtext-toolbar__button';
  summary.textContent = messages.link;
  const form = document.createElement('form');
  form.className = 'richtext-link-form';
  const input = document.createElement('input');
  input.type = 'url';
  input.inputMode = 'url';
  input.placeholder = 'https://';
  input.setAttribute('aria-label', messages.link_url);
  const apply = toolbarButton(messages.apply_link, () => undefined);
  apply.type = 'submit';
  const remove = toolbarButton(messages.remove_link, () => {
    editor.chain().focus().extendMarkRange('link').unsetLink().run();
    details.open = false;
  });
  details.addEventListener('toggle', () => {
    if (details.open) {
      input.value = String(editor.getAttributes('link').href ?? '');
      input.focus();
    }
  });
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const href = input.value.trim();
    if (!href) return;
    editor.chain().focus().extendMarkRange('link').setLink({ href, target: null, rel: null, class: null }).run();
    details.open = false;
  });
  form.append(input, apply, remove);
  details.append(summary, form);
  container.append(details);
}

function toolbarButton(label: string, run: () => void): HTMLButtonElement {
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'richtext-toolbar__button';
  button.textContent = label;
  button.setAttribute('aria-label', label);
  button.addEventListener('click', run);
  return button;
}
