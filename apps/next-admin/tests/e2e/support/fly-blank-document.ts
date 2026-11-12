import type { Page } from '@playwright/test';

const BLANK_DOCUMENT_PATH = '/__fly-e2e-blank-document';
const BLANK_DOCUMENT =
  '<!doctype html><html lang="en"><head><title>Fly adapter harness</title></head><body></body></html>';

// Opens an empty document on the origin under test.
//
// The adapter keeps draft and save-failure state in `sessionStorage`, which needs a real origin:
// `page.setContent` leaves the page on `about:blank`. The document is answered by a route, so no
// page of the host application is loaded. A host page is a React document, and React owns
// `document.body`: when it hydrates or renders after the harness wrote `#fly-root` into the body,
// it regenerates the tree and the root is gone. The route also answers a reload of the document.
export async function openBlankDocument(page: Page): Promise<void> {
  await page.route(`**${BLANK_DOCUMENT_PATH}`, (route) =>
    route.fulfill({ contentType: 'text/html', body: BLANK_DOCUMENT })
  );
  await page.goto(BLANK_DOCUMENT_PATH);
}
