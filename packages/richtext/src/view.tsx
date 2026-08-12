import type { RichTextView } from './generated/contracts';
import { canonicalRichTextLocale } from './authoring';

export interface RichTextHtmlProps {
  view: RichTextView;
  contentLocale: string;
  className?: string;
}

/**
 * Renders the server-derived HTML projection without loading the editor runtime.
 * Raw caller-provided HTML is intentionally not accepted by this boundary.
 */
export function RichTextHtml({
  view,
  contentLocale,
  className
}: RichTextHtmlProps) {
  return (
    <div
      className={className}
      data-richtext=''
      lang={canonicalRichTextLocale(contentLocale)}
      dir='auto'
      dangerouslySetInnerHTML={{ __html: view.html }}
    />
  );
}
