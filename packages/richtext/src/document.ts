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

export interface RichTextValidationOptions {
  /** Allow the editor's canonical empty draft even when the write profile is required. */
  allowEmpty?: boolean;
}

interface ValidationStats {
  nodes: number;
  text: number;
  links: number;
  meaningfulText: boolean;
}

export function emptyRichTextDocument(): RichTextDocument {
  return { type: 'doc', content: [{ type: 'paragraph' }] };
}

export function richTextDocumentHasText(document: RichTextDocument): boolean {
  const stack = [...document.content];
  while (stack.length > 0) {
    const node = stack.pop();
    if (node?.text && [...node.text].some((character) => !/\s/u.test(character))) return true;
    if (node?.content) stack.push(...node.content);
  }
  return false;
}

export function validateRichTextDocument(
  value: unknown,
  profile: RichTextProfileManifest,
  options: RichTextValidationOptions = {}
): RichTextValidationResult {
  let encoded: string;
  try {
    encoded = JSON.stringify(value);
  } catch {
    return invalid('document is not serializable');
  }
  if (byteLength(encoded) > profile.limits.max_json_bytes) {
    return invalid('document exceeds the profile byte limit');
  }
  if (!isRecord(value) || value.type !== 'doc' || !Array.isArray(value.content)) {
    return invalid('document root must be a doc with content');
  }
  if (!hasOnlyKeys(value, ['type', 'content'])) {
    return invalid('document root contains unsupported fields');
  }

  const stats: ValidationStats = {
    nodes: 1,
    text: 0,
    links: 0,
    meaningfulText: false
  };
  if (stats.nodes > profile.limits.max_nodes) return invalid('too many nodes');

  for (const child of value.content) {
    if (!isRecord(child) || typeof child.type !== 'string' || !isBlock(child.type)) {
      return invalid('document root accepts block nodes only');
    }
    const result = validateNode(child, profile, stats, 2);
    if (!result.valid) return result;
  }
  if (!stats.meaningfulText && !profile.allow_empty && !options.allowEmpty) {
    return invalid('document must contain non-whitespace text');
  }
  return { valid: true };
}

function validateNode(
  value: unknown,
  profile: RichTextProfileManifest,
  stats: ValidationStats,
  depth: number
): RichTextValidationResult {
  if (depth > profile.limits.max_depth) return invalid('document is too deep');
  if (!isRecord(value) || typeof value.type !== 'string') return invalid('invalid node');
  if (!hasOnlyKeys(value, ['type', 'attrs', 'content', 'marks', 'text'])) {
    return invalid('node contains unsupported fields');
  }
  if (value.type === 'doc' || !profile.nodes.includes(value.type)) {
    return invalid(`node ${value.type} is not allowed`);
  }

  stats.nodes += 1;
  if (stats.nodes > profile.limits.max_nodes) return invalid('too many nodes');

  const node = value as unknown as RichTextNode;
  if (node.type === 'text') return validateTextNode(node, profile, stats);
  if (node.text !== undefined) return invalid(`${node.type} cannot contain text directly`);

  if (node.type === 'hardBreak') {
    if (!hasEmptyAttrs(node) || !hasNoContent(node)) return invalid('invalid hardBreak structure');
    return validateMarks(node.marks, profile, stats);
  }
  if (!hasNoMarks(node)) return invalid(`${node.type} cannot carry marks`);

  switch (node.type) {
    case 'paragraph':
      if (!hasEmptyAttrs(node)) return invalid('invalid paragraph attributes');
      return validateInlineChildren(node, profile, stats, depth);
    case 'heading':
      if (!validateHeadingAttrs(node, profile)) return invalid('invalid heading attributes');
      return validateInlineChildren(node, profile, stats, depth);
    case 'bulletList':
      if (!hasEmptyAttrs(node)) return invalid('invalid bulletList attributes');
      return validateList(node, profile, stats, depth);
    case 'orderedList':
      if (!validateOrderedListAttrs(node)) return invalid('invalid orderedList attributes');
      return validateList(node, profile, stats, depth);
    case 'listItem':
      if (!hasEmptyAttrs(node)) return invalid('invalid listItem attributes');
      return validateListItem(node, profile, stats, depth);
    case 'blockquote':
      if (!hasEmptyAttrs(node)) return invalid('invalid blockquote attributes');
      return validateBlockChildren(node, profile, stats, depth, true);
    case 'codeBlock':
      if (!hasEmptyAttrs(node)) return invalid('invalid codeBlock attributes');
      return validateCodeBlock(node, profile, stats, depth);
    case 'horizontalRule':
      return hasEmptyAttrs(node) && hasNoContent(node)
        ? { valid: true }
        : invalid('invalid horizontalRule structure');
    default:
      return invalid(`node ${node.type} is not supported`);
  }
}

function validateTextNode(
  node: RichTextNode,
  profile: RichTextProfileManifest,
  stats: ValidationStats
): RichTextValidationResult {
  if (!hasEmptyAttrs(node) || !hasNoContent(node) || typeof node.text !== 'string' || node.text.length === 0) {
    return invalid('invalid text node');
  }
  stats.text += [...node.text].length;
  if (stats.text > profile.limits.max_text_chars) return invalid('too much text');
  stats.meaningfulText ||= [...node.text].some((character) => !/\s/u.test(character));
  return validateMarks(node.marks, profile, stats);
}

function validateMarks(
  marks: RichTextMark[] | undefined,
  profile: RichTextProfileManifest,
  stats: ValidationStats
): RichTextValidationResult {
  if (marks === undefined) return { valid: true };
  if (!Array.isArray(marks) || marks.length > profile.limits.max_marks_per_node) {
    return invalid('invalid marks');
  }
  const seen = new Set<string>();
  for (const mark of marks) {
    if (!isRecord(mark) || typeof mark.type !== 'string' || !hasOnlyKeys(mark, ['type', 'attrs'])) {
      return invalid('invalid mark');
    }
    if (!profile.marks.includes(mark.type)) return invalid(`mark ${mark.type} is not allowed`);
    if (seen.has(mark.type)) return invalid(`duplicate ${mark.type} mark`);
    seen.add(mark.type);

    if (mark.type !== 'link') {
      if (mark.attrs !== undefined && (!isRecord(mark.attrs) || Object.keys(mark.attrs).length > 0)) {
        return invalid(`invalid ${mark.type} attributes`);
      }
      continue;
    }
    if (!isRecord(mark.attrs) || !hasOnlyKeys(mark.attrs, ['href', 'target', 'rel', 'class'])) {
      return invalid('invalid link attributes');
    }
    if (mark.attrs.target != null || mark.attrs.rel != null || mark.attrs.class != null) {
      return invalid('link presentation attributes must be null or absent');
    }
    if (
      typeof mark.attrs.href !== 'string' ||
      byteLength(mark.attrs.href) > profile.limits.max_attribute_bytes ||
      !isSafeHref(mark.attrs.href, profile.limits.max_url_bytes)
    ) {
      return invalid('unsafe link');
    }
    stats.links += 1;
    if (stats.links > profile.limits.max_links) return invalid('too many links');
  }
  return { valid: true };
}

function validateInlineChildren(
  node: RichTextNode,
  profile: RichTextProfileManifest,
  stats: ValidationStats,
  depth: number
): RichTextValidationResult {
  if (node.content !== undefined && !Array.isArray(node.content)) return invalid('invalid node content');
  for (const child of node.content ?? []) {
    if (!isRecord(child) || typeof child.type !== 'string' || !isInline(child.type)) {
      return invalid(`${node.type} accepts inline nodes only`);
    }
    const result = validateNode(child, profile, stats, depth + 1);
    if (!result.valid) return result;
  }
  return { valid: true };
}

function validateBlockChildren(
  node: RichTextNode,
  profile: RichTextProfileManifest,
  stats: ValidationStats,
  depth: number,
  requireChild: boolean
): RichTextValidationResult {
  if (node.content !== undefined && !Array.isArray(node.content)) return invalid('invalid node content');
  const children = node.content ?? [];
  if (requireChild && children.length === 0) return invalid(`${node.type} requires content`);
  for (const child of children) {
    if (!isRecord(child) || typeof child.type !== 'string' || !isBlock(child.type)) {
      return invalid(`${node.type} accepts block nodes only`);
    }
    const result = validateNode(child, profile, stats, depth + 1);
    if (!result.valid) return result;
  }
  return { valid: true };
}

function validateList(
  node: RichTextNode,
  profile: RichTextProfileManifest,
  stats: ValidationStats,
  depth: number
): RichTextValidationResult {
  if (!Array.isArray(node.content) || node.content.length === 0) return invalid(`${node.type} requires content`);
  for (const child of node.content) {
    if (!isRecord(child) || child.type !== 'listItem') return invalid(`${node.type} accepts listItem nodes only`);
    const result = validateNode(child, profile, stats, depth + 1);
    if (!result.valid) return result;
  }
  return { valid: true };
}

function validateListItem(
  node: RichTextNode,
  profile: RichTextProfileManifest,
  stats: ValidationStats,
  depth: number
): RichTextValidationResult {
  if (!Array.isArray(node.content) || node.content[0]?.type !== 'paragraph') {
    return invalid('listItem must start with a paragraph');
  }
  return validateBlockChildren(node, profile, stats, depth, true);
}

function validateCodeBlock(
  node: RichTextNode,
  profile: RichTextProfileManifest,
  stats: ValidationStats,
  depth: number
): RichTextValidationResult {
  if (node.content !== undefined && !Array.isArray(node.content)) return invalid('invalid codeBlock content');
  for (const child of node.content ?? []) {
    if (!isRecord(child) || child.type !== 'text' || (Array.isArray(child.marks) && child.marks.length > 0)) {
      return invalid('codeBlock accepts unmarked text only');
    }
    const result = validateNode(child, profile, stats, depth + 1);
    if (!result.valid) return result;
  }
  return { valid: true };
}

function validateHeadingAttrs(node: RichTextNode, profile: RichTextProfileManifest): boolean {
  return (
    isRecord(node.attrs) &&
    hasOnlyKeys(node.attrs, ['level']) &&
    typeof node.attrs.level === 'number' &&
    Number.isInteger(node.attrs.level) &&
    profile.heading_levels.includes(node.attrs.level)
  );
}

function validateOrderedListAttrs(node: RichTextNode): boolean {
  if (node.attrs === undefined) return true;
  if (!isRecord(node.attrs) || !hasOnlyKeys(node.attrs, ['start'])) return false;
  const start = node.attrs.start;
  return (
    start === undefined ||
    start === null ||
    (typeof start === 'number' && Number.isInteger(start) && start >= 1 && start <= 1_000_000)
  );
}

function hasEmptyAttrs(node: RichTextNode): boolean {
  return node.attrs === undefined || (isRecord(node.attrs) && Object.keys(node.attrs).length === 0);
}

function hasNoContent(node: RichTextNode): boolean {
  return node.content === undefined || (Array.isArray(node.content) && node.content.length === 0);
}

function hasNoMarks(node: RichTextNode): boolean {
  return node.marks === undefined || (Array.isArray(node.marks) && node.marks.length === 0);
}

function isBlock(kind: string): boolean {
  return ['paragraph', 'heading', 'bulletList', 'orderedList', 'blockquote', 'codeBlock', 'horizontalRule'].includes(kind);
}

function isInline(kind: string): boolean {
  return kind === 'text' || kind === 'hardBreak';
}

function isSafeHref(href: string, maxBytes: number): boolean {
  if (
    byteLength(href) > maxBytes ||
    href.trim() !== href ||
    href.length === 0 ||
    /[\u0000-\u001f\u007f]/u.test(href) ||
    href.includes('\\') ||
    href.startsWith('//')
  ) {
    return false;
  }
  if (href.startsWith('/')) return true;
  if (href.startsWith('#')) return href.length > 1;
  try {
    const url = new URL(href);
    if (url.protocol === 'mailto:') return true;
    return (
      (url.protocol === 'http:' || url.protocol === 'https:') &&
      !url.username &&
      !url.password &&
      Boolean(url.hostname)
    );
  } catch {
    return false;
  }
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength;
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
