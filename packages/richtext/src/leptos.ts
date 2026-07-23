import type {
  RichTextDocument,
  RichTextProfileId
} from './generated/contracts';
import type { RichTextMessages } from './messages';
import {
  connectRichTextFrame,
  type RichTextFrameController
} from './frame/controller';

/**
 * Leptos' `on_mount`/`on_cleanup` adapter. The Rust component owns the iframe
 * element and calls this function from its wasm browser binding; all protocol
 * and lifecycle behavior remains in the shared controller.
 */
export function mountLeptosRichTextFrame(
  iframe: HTMLIFrameElement,
  options: {
    frameUrl: string;
    profile: RichTextProfileId;
    document: RichTextDocument;
    messages: RichTextMessages;
    editable?: boolean;
    onDocumentChange(document: RichTextDocument): void;
    onError?(code: string, message: string): void;
  }
): { controller: RichTextFrameController; dispose(): void } {
  const controller = connectRichTextFrame({ iframe, ...options });
  return {
    controller,
    dispose: () => controller.destroy()
  };
}
