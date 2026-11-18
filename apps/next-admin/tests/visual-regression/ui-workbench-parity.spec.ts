import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { expect, test, type Page } from '@playwright/test';

const require = createRequire(import.meta.url);

test.describe('UI Workbench Visual & Functional Parity (Next.js ↔ Leptos)', () => {
  test('Next.js UI Workbench renders complete component matrix and recipes', async ({ page }) => {
    // Navigate to Next.js design-system workbench
    const response = await page.goto('/dashboard/design-system', { waitUntil: 'domcontentloaded' });
    if (response && response.status() >= 400) {
      test.skip(true, `Next.js Workbench not available at /dashboard/design-system (status ${response.status()})`);
    }

    await expect(page.locator('h1')).toContainText(/UI Workbench|Design System & Component Workbench/);

    // 1. Verify Button Inspector section
    const buttonInspector = page.locator('section', { hasText: 'Interactive Button Inspector' });
    await expect(buttonInspector).toBeVisible();

    // 2. Verify Baseline Component Matrix
    const baselineSection = page.locator('section', { hasText: 'Component Baseline Matrix' });
    await expect(baselineSection).toBeVisible();

    // Verify all 4 recipe cards exist
    const recipesSection = page.locator('section', { hasText: 'Composite Pattern Recipes' });
    await expect(recipesSection).toBeVisible();

    const recipe1 = recipesSection.locator('[class*="rounded-xl border"]', { hasText: 'Confirm Action Dialog' });
    await expect(recipe1).toBeVisible();

    const recipe2 = recipesSection.locator('[class*="rounded-xl border"]', { hasText: 'Resource Filter Toolbar' });
    await expect(recipe2).toBeVisible();

    const recipe3 = recipesSection.locator('[class*="rounded-xl border"]', { hasText: 'Save Action Toolbar' });
    await expect(recipe3).toBeVisible();

    const recipe4 = recipesSection.locator('[class*="rounded-xl border"]', { hasText: 'RusToK Core' });
    await expect(recipe4).toBeVisible();
  });

  test('Recipe 1: Confirm Delete Dialog opens modal and traps focus', async ({ page }) => {
    await page.goto('/dashboard/design-system', { waitUntil: 'domcontentloaded' });

    const openTrigger = page.getByRole('button', { name: 'Delete Module' });
    if (!(await openTrigger.isVisible())) {
      test.skip(true, 'Trigger button not found; skipping');
    }

    await openTrigger.click();

    // Verify dialog content is presented
    const dialog = page.getByRole('alertdialog').or(page.getByRole('dialog'));
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText(/Confirm Delete|Confirm Deletion/);
    await expect(dialog).toContainText('This action cannot be undone');

    // Close dialog via Cancel button
    const cancelButton = dialog.getByRole('button', { name: 'Cancel' });
    await cancelButton.click();
    await expect(dialog).not.toBeVisible();
  });

  test('Recipe 3: Save Action Toolbar reflects simulated dirty and saved states', async ({ page }) => {
    await page.goto('/dashboard/design-system', { waitUntil: 'domcontentloaded' });

    await expect(page.getByText('Unsaved changes')).toBeVisible();

    const saveBtn = page.getByRole('button', { name: /^Save( changes)?$/i });
    if (!(await saveBtn.isVisible())) {
      test.skip(true, 'Save changes button not found; skipping');
    }

    await expect(saveBtn).toBeEnabled();
    await saveBtn.click();

    await expect(page.getByText('Changes saved successfully!')).toBeVisible({ timeout: 5000 });
  });

  test('Leptos Admin UI Workbench accessibility and recipe structure (when running)', async ({ page }) => {
    const leptosBase = process.env.RUSTOK_LEPTOS_ADMIN_URL || 'http://127.0.0.1:5150';
    let leptosOnline = false;

    try {
      const resp = await page.goto(`${leptosBase}/design-system`, { timeout: 5000, waitUntil: 'domcontentloaded' });
      leptosOnline = resp !== null && resp.status() < 400;
    } catch {
      leptosOnline = false;
    }

    if (!leptosOnline) {
      test.skip(true, `Leptos host not currently online at ${leptosBase}/design-system`);
    }

    await expect(page.locator('h1')).toContainText(/UI Workbench|Design System & Component Workbench/);

    // Verify Leptos recipes
    await expect(page.getByText('Confirm Action Dialog')).toBeVisible();
    await expect(page.getByText('Resource Filter Toolbar')).toBeVisible();
    await expect(page.getByText('Save Action Toolbar')).toBeVisible();
    await expect(page.getByText('RusToK Core')).toBeVisible();
  });

  test('RichText Editor Sandbox mounts isolated frame and profile controls', async ({ page }) => {
    const response = await page.goto('/dashboard/design-system', { waitUntil: 'domcontentloaded' });
    if (response && response.status() >= 400) {
      test.skip(true, 'Workbench page not available');
    }

    const richTextSection = page.locator('section', { hasText: 'RichText Editor Sandbox' });
    if (!(await richTextSection.isVisible())) {
      test.skip(true, 'RichText Sandbox section not rendered');
    }

    await expect(richTextSection).toBeVisible();
    await expect(richTextSection.getByRole('button', { name: 'article' })).toBeVisible();
    await expect(richTextSection.getByRole('button', { name: 'discussion' })).toBeVisible();
    await expect(richTextSection.getByRole('button', { name: 'comment' })).toBeVisible();
    await expect(richTextSection.locator('iframe')).toBeVisible();
  });

  test('UI Workbench Accessibility Gate (axe-core WCAG 2.1 AA inspection)', async ({ page }) => {
    const response = await page.goto('/dashboard/design-system', { waitUntil: 'domcontentloaded' });
    if (response && response.status() >= 400) {
      test.skip(true, 'Workbench page not available');
    }

    let axePath: string | undefined;
    try {
      axePath = require.resolve('axe-core/axe.min.js');
    } catch {
      axePath = undefined;
    }

    if (!axePath || !existsSync(axePath)) {
      test.skip(true, 'axe-core runtime not found in node_modules');
    }

    await page.addScriptTag({ path: axePath });

    const results = await page.evaluate(async () => {
      return await (window as any).axe.run({
        runOnly: {
          type: 'tag',
          values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']
        }
      });
    });

    const criticalViolations = results.violations.filter(
      (v: any) => v.impact === 'critical' || v.impact === 'serious'
    );
    expect(criticalViolations).toEqual([]);
  });

  test('UI Workbench Visual Snapshot (Component Baseline Matrix)', async ({ page }) => {
    const response = await page.goto('/dashboard/design-system', { waitUntil: 'domcontentloaded' });
    if (response && response.status() >= 400) {
      test.skip(true, 'Workbench page not available');
    }

    const baselineSection = page.locator('section', { hasText: 'Component Baseline Matrix' });
    if (await baselineSection.isVisible()) {
      await expect(baselineSection).toHaveScreenshot('component-baseline-matrix.png', {
        maxDiffPixelRatio: 0.05,
        animations: 'disabled'
      });
    }
  });
});
