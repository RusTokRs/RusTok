export interface RtMark extends Record<string, unknown> {
  type: string;
  attrs?: Record<string, unknown>;
}

export interface RtNode extends Record<string, unknown> {
  type: string;
  attrs?: Record<string, unknown>;
  text?: string;
  marks?: RtMark[];
  content?: RtNode[];
}

export interface RtDoc extends Record<string, unknown> {
  type: 'doc';
  content: RtNode[];
}

export interface RtJsonV1Payload extends Record<string, unknown> {
  version: 'rt_json_v1';
  locale: string;
  doc: RtDoc;
}

const CANONICAL_TO_EDITOR_NODE_TYPES: Record<string, string> = {
  bullet_list: 'bulletList',
  ordered_list: 'orderedList',
  list_item: 'listItem',
  horizontal_rule: 'horizontalRule',
  hard_break: 'hardBreak',
  code_block: 'codeBlock'
};

const EDITOR_TO_CANONICAL_NODE_TYPES = Object.fromEntries(
  Object.entries(CANONICAL_TO_EDITOR_NODE_TYPES).map(([canonical, editor]) => [
    editor,
    canonical
  ])
) as Record<string, string>;

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isRtDoc(value: unknown): value is RtDoc {
  return (
    isObject(value) && value.type === 'doc' && Array.isArray(value.content)
  );
}

function isRtJsonV1Payload(value: unknown): value is RtJsonV1Payload {
  return (
    isObject(value) &&
    value.version === 'rt_json_v1' &&
    typeof value.locale === 'string' &&
    isRtDoc(value.doc)
  );
}

export function normalizeRtJsonPayload(
  value: unknown,
  locale: string
): RtJsonV1Payload {
  const parsed = typeof value === 'string' ? JSON.parse(value) : value;

  if (isRtJsonV1Payload(parsed)) {
    return {
      ...parsed,
      doc: toCanonicalDoc(toEditorDoc(parsed.doc))
    };
  }

  if (isObject(parsed) && isRtDoc(parsed.doc)) {
    return {
      version: 'rt_json_v1',
      locale:
        typeof parsed.locale === 'string' && parsed.locale.trim().length > 0
          ? parsed.locale
          : locale,
      doc: toCanonicalDoc(toEditorDoc(parsed.doc))
    };
  }

  if (isRtDoc(parsed)) {
    return {
      version: 'rt_json_v1',
      locale,
      doc: toCanonicalDoc(parsed)
    };
  }

  throw new Error('Invalid rt_json_v1 document');
}

export function extractRtDoc(value: unknown, locale: string): RtDoc {
  return toEditorDoc(normalizeRtJsonPayload(value, locale).doc);
}

export function stringifyRtJsonPayload(payload: RtJsonV1Payload): string {
  return JSON.stringify(payload, null, 2);
}

export function stringifyRtDoc(doc: RtDoc, locale: string): string {
  return stringifyRtJsonPayload({
    version: 'rt_json_v1',
    locale,
    doc: toCanonicalDoc(doc)
  });
}

function mapNodeType(type: string, mapping: Record<string, string>): string {
  return mapping[type] ?? type;
}

function mapNode(node: RtNode, mapping: Record<string, string>): RtNode {
  return {
    ...node,
    type: mapNodeType(node.type, mapping),
    content: node.content?.map((child) => mapNode(child, mapping))
  };
}

export function toEditorDoc(doc: RtDoc): RtDoc {
  return {
    ...doc,
    type: 'doc',
    content: doc.content.map((node) =>
      mapNode(node, CANONICAL_TO_EDITOR_NODE_TYPES)
    )
  };
}

export function toCanonicalDoc(doc: RtDoc): RtDoc {
  return {
    ...doc,
    type: 'doc',
    content: doc.content.map((node) =>
      mapNode(node, EDITOR_TO_CANONICAL_NODE_TYPES)
    )
  };
}

export function markdownToRtDoc(markdown: string): RtDoc {
  const nodes: RtNode[] = markdown
    .split('\n')
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0)
    .map((line) => {
      if (line.startsWith('## ')) {
        return {
          type: 'heading',
          attrs: { level: 2 },
          content: [{ type: 'text', text: line.slice(3) }]
        };
      }
      if (line.startsWith('# ')) {
        return {
          type: 'heading',
          attrs: { level: 1 },
          content: [{ type: 'text', text: line.slice(2) }]
        };
      }
      if (line.startsWith('- ') || line.startsWith('* ')) {
        return {
          type: 'bulletList',
          content: [
            {
              type: 'listItem',
              content: [
                {
                  type: 'paragraph',
                  content: [{ type: 'text', text: line.slice(2) }]
                }
              ]
            }
          ]
        };
      }
      return { type: 'paragraph', content: [{ type: 'text', text: line }] };
    });

  return { type: 'doc', content: nodes };
}
