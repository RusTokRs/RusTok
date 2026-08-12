import type {
  RichTextDocument,
  RichTextProfileId
} from './generated/contracts';
import type { RichTextMessages } from './messages';
import {
  connectRichTextFrame,
  type RichTextFrameController
} from './frame/controller';
import type {
  RichTextAuthoringContextInput,
  RichTextDirection
} from './authoring';

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
    contentLocale: string;
    direction?: RichTextDirection;
    spellcheck?: boolean;
    editable?: boolean;
    onDocumentChange(document: RichTextDocument): void;
    onError?(code: string, message: string): void;
  }
): {
  controller: RichTextFrameController;
  setDocument(document: RichTextDocument): void;
  setAuthoringContext(input: RichTextAuthoringContextInput): void;
  setEditable(editable: boolean): void;
  dispose(): void;
} {
  const controller = connectRichTextFrame({ iframe, ...options });
  return {
    controller,
    setDocument: (document) => controller.setDocument(document),
    setAuthoringContext: (input) => controller.setAuthoringContext(input),
    setEditable: (editable) => controller.setEditable(editable),
    dispose: () => controller.destroy()
  };
}
