export const CONNECTOR_MODES = [
  {
    mode: 'bundled',
    title: 'Bundled',
    description: 'Use the Iggy artifact installed with this module.'
  },
  {
    mode: 'external',
    title: 'External',
    description: 'Connect to an operator-managed Iggy deployment.'
  }
] as const;

export function isKnownMode(value: string): boolean {
  return CONNECTOR_MODES.some((m) => m.mode === value);
}

export function parseAddresses(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((address) => address.trim())
    .filter(Boolean);
}

export function formatSaveOutcome(restartRequired: boolean): string {
  return restartRequired
    ? 'Saved. Restart the server to activate this connector mode.'
    : 'Saved. This connector mode is already active.';
}
