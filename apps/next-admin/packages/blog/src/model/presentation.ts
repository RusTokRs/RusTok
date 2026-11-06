/** Pure presentation contract shared by owner detail render adapters. */
export function postStatusPresentation(status: string): {
  labelKey:
    'status.draft' | 'status.published' | 'status.archived' | 'status.unknown';
  variant: 'default' | 'secondary' | 'outline';
} {
  switch (status) {
    case 'DRAFT':
      return { labelKey: 'status.draft', variant: 'secondary' };
    case 'PUBLISHED':
      return { labelKey: 'status.published', variant: 'default' };
    case 'ARCHIVED':
      return { labelKey: 'status.archived', variant: 'outline' };
    default:
      return { labelKey: 'status.unknown', variant: 'outline' };
  }
}
