import type { CreatePageInput, PageStarterSummary } from '../api/pages';

export class LayoutSelectionError extends Error {}

/**
 * The client preflight improves feedback; the API create adapter additionally requests
 * checked server-side layout selection. Publish remains authoritative for composition.
 */
export async function createPageFromStarter(
  input: {
    starter: PageStarterSummary;
    locale: string;
    title: string;
    layoutKey: string;
    visibleLayoutKeys: string[];
  },
  ports: {
    instantiate: (starter: PageStarterSummary, locale: string) => Promise<Record<string, unknown>>;
    listLayouts: (locale: string) => Promise<string[]>;
    create: (input: CreatePageInput) => Promise<unknown>;
    contextMatches: () => boolean;
  },
  messages: { layout: string; context: string; contextAfterCreate: string }
): Promise<void> {
  if (!input.visibleLayoutKeys.includes(input.layoutKey)) throw new LayoutSelectionError(messages.layout);
  if (!ports.contextMatches()) throw new Error(messages.context);
  const document = await ports.instantiate(input.starter, input.locale);
  if (!ports.contextMatches()) throw new Error(messages.context);
  const latestKeys = await ports.listLayouts(input.locale);
  if (!ports.contextMatches()) throw new Error(messages.context);
  if (!latestKeys.includes(input.layoutKey)) throw new LayoutSelectionError(messages.layout);
  try {
    await ports.create({ locale: input.locale, title: input.title, template: input.layoutKey, document });
  } catch (error) {
    if (!ports.contextMatches()) throw new Error(messages.contextAfterCreate);
    throw error;
  }
  if (!ports.contextMatches()) throw new Error(messages.contextAfterCreate);
}
