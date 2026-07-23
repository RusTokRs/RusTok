import type {
  RichTextDocument,
  RichTextProfileId
} from '../generated/contracts';
import { getRichTextProfile } from '../profiles';
import type { RichTextMessages } from '../messages';
import {
  MAX_PROTOCOL_OVERHEAD_BYTES,
  RICH_TEXT_PROTOCOL,
  RICH_TEXT_PROTOCOL_REVISION,
  createEnvelope,
  isEnvelope,
  isHandshakeReady,
  type RichTextFrameEvent,
  type RichTextHostCommand
} from '../protocol';

export interface ConnectRichTextFrameOptions {
  iframe: HTMLIFrameElement;
  frameUrl: string;
  profile: RichTextProfileId;
  document: RichTextDocument;
  messages: RichTextMessages;
  editable?: boolean;
  timeoutMs?: number;
  onDocumentChange(document: RichTextDocument): void;
  onFocusChange?(focused: boolean): void;
  onError?(code: string, message: string): void;
}

export class RichTextFrameController {
  readonly session: string;
  readonly ready: Promise<void>;
  private port?: MessagePort;
  private outboundSequence = 0;
  private inboundSequence = 0;
  private destroyed = false;
  private readonly maxMessageBytes: number;
  private resolveReady!: () => void;
  private rejectReady!: (reason: Error) => void;
  private cleanupHandshake?: () => void;

  constructor(private readonly options: ConnectRichTextFrameOptions) {
    this.session = crypto.randomUUID();
    this.maxMessageBytes =
      getRichTextProfile(options.profile).limits.max_json_bytes +
      MAX_PROTOCOL_OVERHEAD_BYTES;
    this.ready = new Promise<void>((resolve, reject) => {
      this.resolveReady = resolve;
      this.rejectReady = reject;
    });
    this.connect();
  }

  setDocument(document: RichTextDocument): void {
    this.send({ type: 'set_document', payload: { document } });
  }

  setEditable(editable: boolean): void {
    this.send({ type: 'set_editable', payload: { editable } });
  }

  focus(): void {
    this.send({ type: 'focus', payload: {} });
  }

  requestDocument(): void {
    this.send({ type: 'request_document', payload: {} });
  }

  destroy(): void {
    if (this.destroyed) return;
    if (this.port) this.send({ type: 'destroy', payload: {} });
    this.destroyed = true;
    this.cleanupHandshake?.();
    this.port?.close();
    this.port = undefined;
  }

  private connect(): void {
    const nonce = crypto.randomUUID();
    const url = new URL(this.options.frameUrl, window.location.href);
    url.hash = new URLSearchParams({ nonce }).toString();
    const timeout = window.setTimeout(() => {
      this.cleanupHandshake?.();
      this.rejectReady(new Error('Richtext frame handshake timed out'));
    }, this.options.timeoutMs ?? 10_000);

    const onMessage = (event: MessageEvent<unknown>) => {
      if (
        event.source !== this.options.iframe.contentWindow ||
        !isHandshakeReady(event.data, nonce)
      ) {
        return;
      }
      window.clearTimeout(timeout);
      window.removeEventListener('message', onMessage);
      const channel = new MessageChannel();
      this.port = channel.port1;
      this.port.onmessage = (portEvent) => this.receive(portEvent.data);
      this.port.start();
      this.options.iframe.contentWindow?.postMessage(
        {
          protocol: RICH_TEXT_PROTOCOL,
          revision: RICH_TEXT_PROTOCOL_REVISION,
          type: 'connect',
          nonce,
          session: this.session
        },
        '*',
        [channel.port2]
      );
      this.send({
        type: 'initialize',
        payload: {
          profile: this.options.profile,
          document: this.options.document,
          messages: this.options.messages,
          editable: this.options.editable ?? true
        }
      });
    };
    this.cleanupHandshake = () => {
      window.clearTimeout(timeout);
      window.removeEventListener('message', onMessage);
    };
    window.addEventListener('message', onMessage);
    this.options.iframe.src = url.toString();
  }

  private send(message: RichTextHostCommand): void {
    if (this.destroyed || !this.port) return;
    this.outboundSequence += 1;
    this.port.postMessage(
      createEnvelope(this.session, this.outboundSequence, message)
    );
  }

  private receive(value: unknown): void {
    if (
      !isEnvelope(
        value,
        this.session,
        this.inboundSequence,
        this.maxMessageBytes
      )
    ) {
      this.options.onError?.('invalid_message', 'The editor frame returned an invalid message.');
      return;
    }
    this.inboundSequence = value.sequence;
    const message = value.message as RichTextFrameEvent;
    switch (message.type) {
      case 'initialized':
        this.resolveReady();
        break;
      case 'document_changed':
      case 'document':
        this.options.onDocumentChange(message.payload.document);
        break;
      case 'focus_changed':
        this.options.onFocusChange?.(message.payload.focused);
        break;
      case 'error':
        this.options.onError?.(message.payload.code, message.payload.message);
        break;
    }
  }
}

export function connectRichTextFrame(
  options: ConnectRichTextFrameOptions
): RichTextFrameController {
  return new RichTextFrameController(options);
}
