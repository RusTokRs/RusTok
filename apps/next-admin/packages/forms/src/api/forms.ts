import { graphqlRequest } from '@/lib/graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export type FormSubmissionState = 'new' | 'read' | 'handled' | 'spam';

export interface FormSubmission {
  id: string;
  formId: string;
  locale: string;
  pageId: string | null;
  payload: Record<string, unknown>;
  state: FormSubmissionState;
  createdAt: string;
  handledAt: string | null;
}

export async function listFormSubmissions(
  filter: {
    formId?: string;
    state?: FormSubmissionState;
    page?: number;
    perPage?: number;
  } = {},
  opts: GqlOpts = {}
): Promise<FormSubmission[]> {
  const data = await graphqlRequest<
    { filter?: Record<string, unknown> },
    { formSubmissions: FormSubmission[] }
  >(
    `
      query FormSubmissions($filter: ListGqlFormSubmissionsFilter) {
        formSubmissions(filter: $filter) {
          id formId locale pageId payload state createdAt handledAt
        }
      }
    `,
    {
      filter: {
        formId: filter.formId ?? null,
        state: filter.state ?? null,
        page: filter.page ?? 1,
        perPage: filter.perPage ?? 20
      }
    },
    opts.token,
    opts.tenantSlug
  );
  return data.formSubmissions;
}

export async function updateFormSubmissionState(
  id: string,
  state: FormSubmissionState,
  opts: GqlOpts = {}
): Promise<FormSubmission> {
  const data = await graphqlRequest<
    { id: string; input: { state: FormSubmissionState } },
    { updateFormSubmissionState: FormSubmission }
  >(
    `
      mutation UpdateFormSubmissionState($id: UUID!, $input: UpdateGqlFormSubmissionStateInput!) {
        updateFormSubmissionState(id: $id, input: $input) {
          id formId locale pageId payload state createdAt handledAt
        }
      }
    `,
    { id, input: { state } },
    opts.token,
    opts.tenantSlug
  );
  return data.updateFormSubmissionState;
}
