import type {
  RichTextDocument,
  RichTextMark,
  RichTextNode,
  RichTextProfileManifest
} from './generated/contracts';

export interface RichTextValidationResult {
  valid: boolean;
  error?: string;
}

export function emptyRichTextDocument(): RichTextDocument {
  return { type: 'doc', content: [{ type: 'paragraph' }] };
}

export function validateRichTextDocument(
  value: unknown,
  profile: RichTextProfileManifest
): RichTextValidationResult {
  let encoded: string;
  try {
    encoded = JSON.stringify(value);
  } catch {
    return invalid('document is not serializable');
  }
  if (new TextEncoder().encode(encoded).byteLength > profile.limits.max_json_bytes) {
    return invalid('document exceeds the profile byte limit');
  }
  if (!isRecord(value) || value.type !== 'doc' || !Array.isArray(value.content)) {
    return invalid('document root must be a doc with content');
  }
  if (!hasOnlyKeys(value, ['type', 'content'])) {
    return invalid('document root contains unsupported fields');
  }

  const stats = { nodes: 0, text: 0, links: 0 };
  for (let index = 0; index < value.content.length; index += 1) {
    const result = validateNode(value.content[index], profile, stats, 1);
    if (!result.valid) return result;
  }
  if (stats.nodes > profile.limits.max_nodes) return invalid('too many nodes');
  if (stats.text > profile.limits.max_text_chars) return invalid('too much text');
  if (stats.links > profile.limits.max_links) return invalid('too many links');
  return { valid: true };
}

function validateNode(
  value: unknown,
  profile: RichTextProfileManifest,
  stats: { nodes: number; text: number; links: number },
  depth: number
): RichTextValidationResult {
  if (depth > profile.limits.max_depth) return invalid('document is too deep');
  if (!isRecord(value) || typeof value.type !== 'string') return invalid('invalid node');
  if (!hasOnlyKeys(value, ['type', 'attrs', 'content', 'marks', 'text'])) {
    return invalid('node contains unsupported fields');
  }
  if (!profile.nodes.includes(value.type)) return invalid(`node ${value.type} is not allowed`);
  stats.nodes += 1;

  const node = value as unknown as RichTextNode;
  if (node.text !== undefined) {
    if (node.type !== 'text' || typeof node.text !== 'string') return invalid('invalid text node');
    stats.text += [...node.text].length;
  }
  if (!validateNodeAttrs(node, profile)) return invalid(`invalid ${node.type} attributes`);
  if (node.marks !== undefined) {
    if (!Array.isArray(node.marks) || node.marks.length > profile.limits.max_marks_per_node) {
      return invalid('invalid marks');
    }
    for (const mark of node.marks) {
      const result = validateMark(mark, profile, stats);
      if (!result.valid) return result;
    }
  }
  if (node.content !== undefined) {
    if (!Array.isArray(node.content)) return invalid('invalid node content');
    for (const child of node.content) {
      const result = validateNode(child, profile, stats, depth + 1);
      if (!result.valid) return result;
    }
  }
  return { valid: true };
}

function validateNodeAttrs(node: RichTextNode, profile: RichTextProfileManifest): boolean {
  const attrs = node.attrs;
  if (attrs === undefined) return node.type !== 'heading';
  if (!isRecord(attrs)) return false;
  if (node.type === 'heading') {
    return hasOnlyKeys(attrs, ['level']) && typeof attrs.level === 'number' && profile.heading_levels.includes(attrs.level);
  }
  if (node.type === 'orderedList') {
    return hasOnlyKeys(attrs, ['start']) && (attrs.start === undefined || (Number.isInteger(attrs.start) && Number(attrs.start) >= 1));
  }
  return Object.keys(attrs).length === 0;
}

function validateMark(
  value: unknown,
  profile: RichTextProfileManifest,
  stats: { links: number }
): RichTextValidationResult {
  if (!isRecord(value) || typeof value.type !== 'string' || !hasOnlyKeys(value, ['type', 'attrs'])) {
    return invalid('invalid mark');
  }
  if (!profile.marks.includes(value.type)) return invalid(`mark ${value.type} is not allowed`);
  const mark = value as unknown as RichTextMark;
  if (mark.type !== 'link') {
    return mark.attrs === undefined || (isRecord(mark.attrs) && Object.keys(mark.attrs).length === 0)
      ? { valid: true }
      : invalid(`invalid ${mark.type} attributes`);
  }
  if (!isRecord(mark.attrs) || !hasOnlyKeys(mark.attrs, ['href', 'target', 'rel', 'class'])) {
    return invalid('invalid link attributes');
  }
  if (mark.attrs.target != null || mark.attrs.rel != null || mark.attrs.class != null) {
    return invalid('link presentation attributes must be null or absent');
  }
  if (typeof mark.attrs.href !== 'string' || !isSafeHref(mark.attrs.href, profile.limits.max_url_bytes)) {
    return invalid('unsafe link');
  }
  stats.links += 1;
  return { valid: true };
}

function isSafeHref(href: string, maxBytes: number): boolean {
  if (new TextEncoder().encode(href).byteLength > maxBytes || href.trim() !== href) return false;
  if (href.startsWith('/') && !href.startsWith('//')) return true;
  try {
    const url = new URL(href);
    return (url.protocol === 'http:' || url.protocol === 'https:' || url.protocol === 'mailto:') && !url.username && !url.password;
  } catch {
    return false;
  }
}

function hasOnlyKeys(value: Record<string, unknown>, allowed: string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function invalid(error: string): RichTextValidationResult {
  return { valid: false, error };
}
