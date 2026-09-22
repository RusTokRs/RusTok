import { mountLeptosRichTextFrame } from './leptos';
import type {
  RichTextDocument,
  RichTextProfileId
} from './generated/contracts';
import type { RichTextMessages } from './messages';
import type { RichTextAuthoringContextInput } from './authoring';

type RichTextHandle = {
  setDocument(document: RichTextDocument): void;
  setAuthoringContext(input: RichTextAuthoringContextInput): void;
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
        contentLocale: string,
        spellcheck: boolean,
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
      setLeptosRichTextAuthoringContext(
        handle: RichTextHandle,
        contentLocale: string,
        spellcheck: boolean
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
    contentLocale,
    spellcheck,
    editable,
    onDocumentChange,
    onError
  ) {
    const handle = mountLeptosRichTextFrame(iframe, {
      frameUrl,
      profile,
      document: JSON.parse(documentJson),
      messages: JSON.parse(messagesJson) as RichTextMessages,
      contentLocale,
      spellcheck,
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
  setLeptosRichTextAuthoringContext(handle, contentLocale, spellcheck) {
    handle.setAuthoringContext({ contentLocale, spellcheck });
  },
  disposeLeptosRichTextFrame(handle) {
    handle.dispose();
  }
};
