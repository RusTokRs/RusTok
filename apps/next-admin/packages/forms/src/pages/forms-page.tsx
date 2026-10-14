import type { FormSubmission, GqlOpts } from '../api/forms';
import { listFormSubmissions } from '../api/forms';
import { SubmissionActions } from '../components/submission-actions';

interface FormsPageProps {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

/**
 * Form-submission inbox for the classic Next admin: the submissions collected by
 * builder/front-end forms, triaged through the same rustok-forms GraphQL state
 * machine (new → read → handled / spam) the Rust FFA admin uses.
 */
export default async function FormsPage({ token, tenantSlug, tenantId }: FormsPageProps) {
  const opts: GqlOpts = { token, tenantSlug, tenantId };
  let items: FormSubmission[] = [];
  let loadError: string | null = null;
  try {
    items = await listFormSubmissions({ page: 1, perPage: 50 }, opts);
  } catch (cause) {
    loadError = cause instanceof Error ? cause.message : 'Failed to load submissions';
  }

  return (
    <section>
      <h1>Form submissions</h1>
      {loadError ? (
        <p role="alert">Submissions could not be loaded: {loadError}</p>
      ) : items.length === 0 ? (
        <p>No submissions yet.</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Received</th>
              <th>Form</th>
              <th>Locale</th>
              <th>State</th>
              <th>Payload</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {items.map((submission) => (
              <tr key={submission.id}>
                <td>{submission.createdAt}</td>
                <td>{submission.formId}</td>
                <td>{submission.locale}</td>
                <td>{submission.state}</td>
                <td>
                  <code>{JSON.stringify(submission.payload)}</code>
                </td>
                <td>
                  <SubmissionActions
                    opts={opts}
                    submissionId={submission.id}
                    state={submission.state}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
