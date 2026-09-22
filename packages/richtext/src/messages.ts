export interface RichTextMessages {
  bold: string;
  italic: string;
  strike: string;
  code: string;
  heading: string;
  bullet_list: string;
  ordered_list: string;
  blockquote: string;
  code_block: string;
  horizontal_rule: string;
  link: string;
  link_url: string;
  apply_link: string;
  remove_link: string;
  clear_formatting: string;
  undo: string;
  redo: string;
  editor: string;
}

export const RICH_TEXT_MESSAGE_KEYS = [
  'bold',
  'italic',
  'strike',
  'code',
  'heading',
  'bullet_list',
  'ordered_list',
  'blockquote',
  'code_block',
  'horizontal_rule',
  'link',
  'link_url',
  'apply_link',
  'remove_link',
  'clear_formatting',
  'undo',
  'redo',
  'editor'
] as const;

export function isRichTextMessages(value: unknown): value is RichTextMessages {
  if (!isRecord(value)) return false;
  return RICH_TEXT_MESSAGE_KEYS.every(
    (key) => typeof value[key] === 'string' && value[key].length > 0
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
