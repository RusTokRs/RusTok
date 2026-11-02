/** Client projection of rustok-web's canonical flat HTTP error envelope. */

export type HttpRetryableState = 'owner' | 'absent' | 'invalid';

export class HttpError extends Error {
  public readonly status: number;
  public readonly code?: string;
  public readonly retryable?: boolean;
  public readonly retryableState: HttpRetryableState;

  constructor(status: number, fallbackMessage: string, payload: unknown) {
    let message = fallbackMessage;
    let code: string | undefined;
    let retryable: boolean | undefined;
    let retryableState: HttpRetryableState = 'invalid';
    if (payload && typeof payload === 'object' && !Array.isArray(payload)) {
      const body = payload as Record<string, unknown>;
      if (typeof body.code === 'string' && typeof body.message === 'string') {
        code = body.code;
        if (body.message.trim().length > 0) message = body.message;
        if (!Object.hasOwn(body, 'retryable')) {
          retryableState = 'absent';
        } else if (typeof body.retryable === 'boolean') {
          retryable = body.retryable;
          retryableState = 'owner';
        }
      }
    }
    super(message);
    this.name = 'HttpError';
    this.status = status;
    this.code = code;
    this.retryable = retryable;
    this.retryableState = retryableState;
  }
}

const MAX_ERROR_BODY_BYTES = 16_384;
const MAX_ERROR_BODY_WAIT_MS = 2_000;

/** Decode up to 16 KiB within two seconds; unsupported bodies are never public text. */
async function readErrorPayload(response: Response): Promise<unknown> {
  let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    if (!response.body) return undefined;
    const activeReader = response.body.getReader();
    reader = activeReader;
    const cancelRead = () => {
      // The HTTP failure remains observable even if upstream cancellation fails or stalls.
      void activeReader.cancel().catch(() => undefined);
    };
    const payload = (async () => {
      const chunks: Uint8Array[] = [];
      let length = 0;
      while (true) {
        const { done, value } = await activeReader.read();
        if (done) break;
        if (value.byteLength === 0) continue;
        length += value.byteLength;
        if (length > MAX_ERROR_BODY_BYTES) {
          cancelRead();
          return undefined;
        }
        chunks.push(value.slice());
      }
      const bytes = new Uint8Array(length);
      let offset = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      return JSON.parse(
        new TextDecoder('utf-8', { fatal: true }).decode(bytes)
      );
    })();
    const expired = new Promise<undefined>((resolve) => {
      timer = setTimeout(() => {
        resolve(undefined);
        cancelRead();
      }, MAX_ERROR_BODY_WAIT_MS);
    });
    return await Promise.race([payload, expired]);
  } catch {
    // Unreadable advice is invalid evidence, not an owner refusal or permission to replay.
    return undefined;
  } finally {
    if (timer !== undefined) clearTimeout(timer);
    reader?.releaseLock();
  }
}

/**
 * Read one failed response without another request, status classification, or mutation replay.
 * Owner retry advice is not a durable outcome and never authorizes a new idempotency key.
 */
export async function readHttpError(
  response: Response,
  fallbackMessage: string
): Promise<HttpError> {
  return new HttpError(
    response.status,
    fallbackMessage,
    await readErrorPayload(response)
  );
}
