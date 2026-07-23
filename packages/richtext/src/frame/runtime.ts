import type { Editor } from '@tiptap/core';
import {
  createRichTextEditor,
  setEditorDocument
} from '../editor';
import type {
  RichTextDocument,
  RichTextProfileManifest
} from '../generated/contracts';
import { validateRichTextDocument } from '../document';
import { isRichTextMessages, type RichTextMessages } from '../messages';
import {
  MAX_PROTOCOL_OVERHEAD_BYTES,
  RICH_TEXT_PROTOCOL,
  RICH_TEXT_PROTOCOL_REVISION,
  createEnvelope,
  isEnvelope,
  isHandshakeConnect,
  isRecord,
  type RichTextFrameEvent,
  type RichTextHostCommand
} from '../protocol';
import { getRichTextProfile, isRichTextProfileId } from '../profiles';
import { mountToolbar } from '../toolbar';

const nonce = new URLSearchParams(window.location.hash.slice(1)).get('nonce');
const maximumDocumentBytes = Math.max(
  ...['article', 'discussion', 'comment'].map(
    (id) => getRichTextProfile(id as 'article').limits.max_json_bytes
  )
);

if (!nonce || nonce.length < 16) {
  throw new Error('Missing richtext frame nonce');
}
history.replaceState(null, '', `${location.pathname}${location.search}`);

let port: MessagePort | undefined;
let session = '';
let editor: Editor | undefined;
let profile: RichTextProfileManifest | undefined;
let messages: RichTextMessages | undefined;
let inboundSequence = 0;
let outboundSequence = 0;
let unmountToolbar: (() => void) | undefined;

const toolbarElement = requireElement('richtext-toolbar');
const editorElement = requireElement('richtext-editor');

const onWindowMessage = (event: MessageEvent<unknown>) => {
  if (
    event.source !== window.parent ||
    !isHandshakeConnect(event.data, nonce) ||
    event.ports.length !== 1 ||
    port
  ) {
    return;
  }
  session = event.data.session;
  port = event.ports[0];
  port.onmessage = (portEvent) => receive(portEvent.data);
  port.start();
  window.removeEventListener('message', onWindowMessage);
};
window.addEventListener('message', onWindowMessage);
window.parent.postMessage(
  {
    protocol: RICH_TEXT_PROTOCOL,
    revision: RICH_TEXT_PROTOCOL_REVISION,
    type: 'ready',
    nonce
  },
  '*'
);

function receive(value: unknown): void {
  const maxBytes =
    (profile?.limits.max_json_bytes ?? maximumDocumentBytes) +
    MAX_PROTOCOL_OVERHEAD_BYTES;
  if (!isEnvelope(value, session, inboundSequence, maxBytes)) {
    sendError('invalid_message', 'The host sent an invalid editor message.');
    return;
  }
  inboundSequence = value.sequence;
  const message = value.message as RichTextHostCommand;
  switch (message.type) {
    case 'initialize':
      initialize(message.payload);
      break;
    case 'set_document':
      applyDocument(message.payload?.document);
      break;
    case 'set_editable':
      if (!editor || typeof message.payload?.editable !== 'boolean') return sendError('not_initialized', 'The editor is not initialized.');
      editor.setEditable(message.payload.editable);
      break;
    case 'focus':
      editor?.commands.focus();
      break;
    case 'request_document':
      if (editor) send({ type: 'document', payload: { document: editor.getJSON() as RichTextDocument } });
      break;
    case 'destroy':
      destroy();
      break;
    default:
      sendError('unsupported_message', 'The host sent an unsupported editor command.');
  }
}

function initialize(payload: unknown): void {
  if (editor || !isRecord(payload)) return sendError('invalid_initialize', 'The editor initialization payload is invalid.');
  if (!isRichTextProfileId(payload.profile) || !isRichTextMessages(payload.messages) || typeof payload.editable !== 'boolean') {
    return sendError('invalid_initialize', 'The editor initialization payload is invalid.');
  }
  const selectedProfile = getRichTextProfile(payload.profile);
  const validation = validateRichTextDocument(payload.document, selectedProfile);
  if (!validation.valid) return sendError('invalid_document', validation.error ?? 'The document is invalid.');

  profile = selectedProfile;
  messages = payload.messages;
  editor = createRichTextEditor({
    element: editorElement,
    profile,
    document: payload.document as RichTextDocument,
    editable: payload.editable,
    onChange: (document) => {
      const result = validateRichTextDocument(document, profile!);
      if (result.valid) send({ type: 'document_changed', payload: { document } });
      else sendError('invalid_editor_document', result.error ?? 'The editor produced an invalid document.');
    },
    onFocusChange: (focused) => send({ type: 'focus_changed', payload: { focused } })
  });
  editorElement.setAttribute('aria-label', messages.editor);
  unmountToolbar = mountToolbar(toolbarElement, editor, profile, messages);
  send({ type: 'initialized', payload: { document: editor.getJSON() as RichTextDocument } });
}

function applyDocument(document: unknown): void {
  if (!editor || !profile) return sendError('not_initialized', 'The editor is not initialized.');
  const validation = validateRichTextDocument(document, profile);
  if (!validation.valid) return sendError('invalid_document', validation.error ?? 'The document is invalid.');
  setEditorDocument(editor, document as RichTextDocument);
}

function send(message: RichTextFrameEvent): void {
  if (!port) return;
  outboundSequence += 1;
  port.postMessage(createEnvelope(session, outboundSequence, message));
}

function sendError(code: string, message: string): void {
  send({ type: 'error', payload: { code, message } });
}

function destroy(): void {
  unmountToolbar?.();
  editor?.destroy();
  editor = undefined;
  port?.close();
  port = undefined;
}

function requireElement(id: string): HTMLElement {
  const element = document.getElementById(id);
  if (!element) throw new Error(`Missing frame element: ${id}`);
  return element;
}
