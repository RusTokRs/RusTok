import { mountLeptosRichTextFrame } from './leptos';
import type { RichTextProfileId } from './generated/contracts';
import type { RichTextMessages } from './messages';

type RichTextHandle = { dispose(): void };

declare global {
  interface Window {
    RustokRichText?: {
      mountLeptosRichTextFrame(
        iframe: HTMLIFrameElement,
        frameUrl: string,
        profile: RichTextProfileId,
        documentJson: string,
        messagesJson: string,
        editable: boolean,
        onDocumentChange: (documentJson: string) => void,
        onError: (code: string, message: string) => void
      ): RichTextHandle;
      disposeLeptosRichTextFrame(handle: RichTextHandle): void;
    };
  }
}

window.RustokRichText = {
  mountLeptosRichTextFrame(
    iframe,
    frameUrl,
    profile,
    documentJson,
    messagesJson,
    editable,
    onDocumentChange,
    onError
  ) {
    return mountLeptosRichTextFrame(iframe, {
      frameUrl,
      profile,
      document: JSON.parse(documentJson),
      messages: JSON.parse(messagesJson) as RichTextMessages,
      editable,
      onDocumentChange: (document) => onDocumentChange(JSON.stringify(document)),
      onError
    });
  },
  disposeLeptosRichTextFrame(handle) {
    handle.dispose();
  }
};

