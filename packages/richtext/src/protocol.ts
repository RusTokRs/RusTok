import type {
  RichTextDocument,
  RichTextProfileId
} from './generated/contracts';
import type { RichTextMessages } from './messages';

export const RICH_TEXT_PROTOCOL = 'rustok.richtext';
export const RICH_TEXT_PROTOCOL_REVISION = 1;
export const MAX_PROTOCOL_OVERHEAD_BYTES = 16 * 1024;

export interface RichTextHandshakeReady {
  protocol: typeof RICH_TEXT_PROTOCOL;
  revision: typeof RICH_TEXT_PROTOCOL_REVISION;
  type: 'ready';
  nonce: string;
}

export interface RichTextHandshakeConnect {
  protocol: typeof RICH_TEXT_PROTOCOL;
  revision: typeof RICH_TEXT_PROTOCOL_REVISION;
  type: 'connect';
  nonce: string;
  session: string;
}

export interface RichTextInitializePayload {
  profile: RichTextProfileId;
  document: RichTextDocument;
  messages: RichTextMessages;
  editable: boolean;
}

export type RichTextHostCommand =
  | { type: 'initialize'; payload: RichTextInitializePayload }
  | { type: 'set_document'; payload: { document: RichTextDocument } }
  | { type: 'set_editable'; payload: { editable: boolean } }
  | { type: 'focus'; payload: Record<string, never> }
  | { type: 'request_document'; payload: Record<string, never> }
  | { type: 'destroy'; payload: Record<string, never> };

export type RichTextFrameEvent =
  | { type: 'initialized'; payload: { document: RichTextDocument } }
  | { type: 'document_changed'; payload: { document: RichTextDocument } }
  | { type: 'document'; payload: { document: RichTextDocument } }
  | { type: 'focus_changed'; payload: { focused: boolean } }
  | { type: 'error'; payload: { code: string; message: string } };

export interface RichTextEnvelope<T> {
  protocol: typeof RICH_TEXT_PROTOCOL;
  revision: typeof RICH_TEXT_PROTOCOL_REVISION;
  session: string;
  sequence: number;
  message: T;
}

export function createEnvelope<T>(
  session: string,
  sequence: number,
  message: T
): RichTextEnvelope<T> {
  return {
    protocol: RICH_TEXT_PROTOCOL,
    revision: RICH_TEXT_PROTOCOL_REVISION,
    session,
    sequence,
    message
  };
}

export function isEnvelope(
  value: unknown,
  session: string,
  lastSequence: number,
  maxBytes: number
): value is RichTextEnvelope<unknown> {
  if (measureMessage(value) > maxBytes || !isRecord(value)) return false;
  return (
    value.protocol === RICH_TEXT_PROTOCOL &&
    value.revision === RICH_TEXT_PROTOCOL_REVISION &&
    value.session === session &&
    Number.isSafeInteger(value.sequence) &&
    Number(value.sequence) > lastSequence &&
    isRecord(value.message)
  );
}

export function measureMessage(value: unknown): number {
  try {
    return new TextEncoder().encode(JSON.stringify(value)).byteLength;
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}

export function isHandshakeReady(
  value: unknown,
  nonce: string
): value is RichTextHandshakeReady {
  return (
    isRecord(value) &&
    value.protocol === RICH_TEXT_PROTOCOL &&
    value.revision === RICH_TEXT_PROTOCOL_REVISION &&
    value.type === 'ready' &&
    value.nonce === nonce
  );
}

export function isHandshakeConnect(
  value: unknown,
  nonce: string
): value is RichTextHandshakeConnect {
  return (
    isRecord(value) &&
    value.protocol === RICH_TEXT_PROTOCOL &&
    value.revision === RICH_TEXT_PROTOCOL_REVISION &&
    value.type === 'connect' &&
    value.nonce === nonce &&
    typeof value.session === 'string' &&
    value.session.length >= 16
  );
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
