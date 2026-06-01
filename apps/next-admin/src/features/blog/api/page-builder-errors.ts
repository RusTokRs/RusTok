import { GraphqlError } from '@/lib/graphql';

export const PAGE_BUILDER_ERROR_CATALOG = {
  validation: 'validation',
  sanitize: 'sanitize',
  runtime: 'runtime',
  featureDisabled: 'feature-disabled'
} as const;

export const PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE = 'FEATURE_DISABLED';

export type PageBuilderErrorKind =
  (typeof PAGE_BUILDER_ERROR_CATALOG)[keyof typeof PAGE_BUILDER_ERROR_CATALOG];

export interface PageBuilderErrorViewModel {
  kind: PageBuilderErrorKind;
  message: string;
  operatorGuidance: string;
}

const DEFAULT_RUNTIME_MESSAGE = 'Failed to save page project';
const FEATURE_DISABLED_GUIDANCE =
  'Builder publish is disabled for this tenant. Keep the page readable, ask Platform to check builder.publish.enabled, and use the tenant rollback/change-set runbook if this was unexpected.';

export function resolvePageBuilderError(
  error: unknown
): PageBuilderErrorViewModel {
  if (
    error instanceof GraphqlError &&
    error.code === PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE
  ) {
    return {
      kind: PAGE_BUILDER_ERROR_CATALOG.featureDisabled,
      message: error.message || 'Page builder capability is disabled',
      operatorGuidance: FEATURE_DISABLED_GUIDANCE
    };
  }

  if (error instanceof Error) {
    const lower = error.message.toLowerCase();
    if (lower.includes('sanitize') || lower.includes('sanitization')) {
      return {
        kind: PAGE_BUILDER_ERROR_CATALOG.sanitize,
        message: error.message,
        operatorGuidance:
          'Review the GrapesJS payload for unsafe HTML, scripts, URLs, or attributes before retrying.'
      };
    }

    if (lower.includes('validation') || lower.includes('invalid')) {
      return {
        kind: PAGE_BUILDER_ERROR_CATALOG.validation,
        message: error.message,
        operatorGuidance:
          'Check required page fields and ensure project data is valid grapesjs_v1 JSON.'
      };
    }

    return {
      kind: PAGE_BUILDER_ERROR_CATALOG.runtime,
      message: error.message,
      operatorGuidance:
        'Retry after checking server health, tenant context, and page-builder runtime logs.'
    };
  }

  return {
    kind: PAGE_BUILDER_ERROR_CATALOG.runtime,
    message: DEFAULT_RUNTIME_MESSAGE,
    operatorGuidance:
      'Retry after checking server health, tenant context, and page-builder runtime logs.'
  };
}
