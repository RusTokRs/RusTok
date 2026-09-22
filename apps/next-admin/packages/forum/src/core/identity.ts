const FORUM_UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function isForumUuid(value: string): boolean {
  return FORUM_UUID_PATTERN.test(value);
}

export function newForumUuid(): string {
  if (globalThis.crypto?.randomUUID) {
    return globalThis.crypto.randomUUID();
  }
  const now = BigInt(Date.now());
  const entropy = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
  const value = ((now << BigInt(64)) ^ entropy)
    .toString(16)
    .padStart(32, '0')
    .slice(-32);
  return `${value.slice(0, 8)}-${value.slice(8, 12)}-4${value.slice(
    13,
    16
  )}-8${value.slice(17, 20)}-${value.slice(20)}`;
}
