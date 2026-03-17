export interface RtTextNode {
  type: 'text';
  text: string;
}

export interface RtParagraphNode {
  type: 'paragraph';
  content?: RtTextNode[];
}

export interface RtHeadingNode {
  type: 'heading';
  attrs: { level: number };
  content?: RtTextNode[];
}

export type RtNode = RtParagraphNode | RtHeadingNode;

export interface RtDoc {
  type: 'doc';
  content: RtNode[];
}

export function parseRtDoc(value: unknown): RtDoc {
  const parsed = typeof value === 'string' ? JSON.parse(value) : value;
  if (!parsed || typeof parsed !== 'object' || (parsed as RtDoc).type !== 'doc') {
    throw new Error('Invalid rt_json_v1 document');
  }
  return parsed as RtDoc;
}

export function stringifyRtDoc(doc: RtDoc): string {
  return JSON.stringify(doc, null, 2);
}

export function markdownToRtDoc(markdown: string): RtDoc {
  const nodes: RtNode[] = markdown
    .split('\n')
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0)
    .map((line) => {
      if (line.startsWith('## ')) {
        return { type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: line.slice(3) }] };
      }
      if (line.startsWith('# ')) {
        return { type: 'heading', attrs: { level: 1 }, content: [{ type: 'text', text: line.slice(2) }] };
      }
      if (line.startsWith('- ') || line.startsWith('* ')) {
        return { type: 'paragraph', content: [{ type: 'text', text: `• ${line.slice(2)}` }] };
      }
      return { type: 'paragraph', content: [{ type: 'text', text: line }] };
    });

  return { type: 'doc', content: nodes };
}
