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

export interface FormSubmissionsFilter {
  formId?: string;
  state?: FormSubmissionState;
  page?: number;
  perPage?: number;
}

export const FORM_SUBMISSIONS_QUERY = `
query FormSubmissions($filter: ListGqlFormSubmissionsFilter) {
  formSubmissions(filter: $filter) {
    id formId locale pageId payload state createdAt handledAt
  }
}
`;

export const UPDATE_FORM_SUBMISSION_STATE_MUTATION = `
mutation UpdateFormSubmissionState($id: UUID!, $input: UpdateGqlFormSubmissionStateInput!) {
  updateFormSubmissionState(id: $id, input: $input) {
    id formId locale pageId payload state createdAt handledAt
  }
}
`;

export async function listFormSubmissions(
  filter: FormSubmissionsFilter = {},
  opts: GqlOpts = {}
): Promise<FormSubmission[]> {
  const data = await graphqlRequest<
    { filter?: Record<string, unknown> },
    { formSubmissions: FormSubmission[] }
  >(
    FORM_SUBMISSIONS_QUERY,
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
    UPDATE_FORM_SUBMISSION_STATE_MUTATION,
    { id, input: { state } },
    opts.token,
    opts.tenantSlug
  );
  return data.updateFormSubmissionState;
}
