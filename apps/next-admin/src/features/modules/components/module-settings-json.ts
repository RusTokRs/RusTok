import { isLosslessNumber, parse, stringify } from 'lossless-json';

/** Preserve every JSON number while displaying or submitting a settings document. */
export function formatSettingsJson(text: string, pretty: boolean): string {
  const result = stringify(parse(text), null, pretty ? 2 : undefined);
  if (result === undefined) throw new Error('Settings are not serializable');
  return result;
}

/** Parse the editor's object without converting its numbers to JavaScript numbers. */
export function parseSettingsObject(text: string): Record<string, unknown> {
  const value = parse(text);
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    isLosslessNumber(value)
  ) {
    throw new Error('Settings must be a JSON object');
  }
  return value as Record<string, unknown>;
}

/** Unsigned decimal JSON integer, retained as a lexical number, not a JS number. */
export function parseSettingsUnsignedInteger(text: string) {
  if (!/^(0|[1-9][0-9]*)$/.test(text)) {
    throw new Error('Expected an unsigned decimal integer');
  }
  return parse(text);
}

/** Update object fields without rounding unrelated numeric values. */
export function mergeSettingsJson(
  text: string,
  updates: Record<string, unknown>
): string {
  const value = parseSettingsObject(text);
  const result = stringify({ ...value, ...updates }, null, 2);
  if (result === undefined) throw new Error('Settings are not serializable');
  return result;
}
