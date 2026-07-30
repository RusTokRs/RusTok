import { mountLeptosRichTextFrame } from './leptos';
import type {
  RichTextDocument,
  RichTextProfileId
} from './generated/contracts';
import type { RichTextMessages } from './messages';

type RichTextHandle = {
  setDocument(document: RichTextDocument): void;
  setEditable(editable: boolean): void;
  dispose(): void;
};

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
      setLeptosRichTextDocument(
        handle: RichTextHandle,
        documentJson: string
      ): void;
      setLeptosRichTextEditable(
        handle: RichTextHandle,
        editable: boolean
      ): void;
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
    const handle = mountLeptosRichTextFrame(iframe, {
      frameUrl,
      profile,
      document: JSON.parse(documentJson),
      messages: JSON.parse(messagesJson) as RichTextMessages,
      editable,
      onDocumentChange: (document) => onDocumentChange(JSON.stringify(document)),
      onError
    });
    handle.controller.ready.catch((error: unknown) => {
      onError(
        'frame_unavailable',
        error instanceof Error
          ? error.message
          : 'The editor frame failed to load.'
      );
    });
    return handle;
  },
  setLeptosRichTextDocument(handle, documentJson) {
    handle.setDocument(JSON.parse(documentJson) as RichTextDocument);
  },
  setLeptosRichTextEditable(handle, editable) {
    handle.setEditable(editable);
  },
  disposeLeptosRichTextFrame(handle) {
    handle.dispose();
  }
};
