import type { RichTextDocument } from './generated/contracts';

export interface DraftStorage {
  saveDraft(key: string, doc: RichTextDocument): void;
  loadDraft(key: string): RichTextDocument | undefined;
  clearDraft(key: string): void;
}

export const localStorageDraftStorage: DraftStorage = {
  saveDraft(key, doc) {
    localStorage.setItem(`richtext_draft_${key}`, JSON.stringify({
      timestamp: Date.now(),
      doc
    }));
  },
  loadDraft(key) {
    const item = localStorage.getItem(`richtext_draft_${key}`);
    if (!item) return undefined;
    try {
      const { doc } = JSON.parse(item);
      return doc as RichTextDocument;
    } catch {
      return undefined;
    }
  },
  clearDraft(key) {
    localStorage.removeItem(`richtext_draft_${key}`);
  }
};
