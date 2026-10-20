/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { ProductWriteJournal } from './write-journal.ts';
export const testScope = { tenantId: 'tenant', actorId: 'actor' };
export const testIntent = 'd698ac66-3906-42e7-9b7c-885f58ba341c';
export function memoryJournal(
  resource = 'new',
  state = { text: null, locked: false },
  scope = testScope,
  intent = testIntent
) {
  return new ProductWriteJournal(scope, resource, intent, {
    read: () => state.text,
    write: (value) => {
      state.text = value;
    },
    exclusive: async (work) => {
      if (state.locked) throw new Error('busy');
      state.locked = true;
      try {
        return await work();
      } finally {
        state.locked = false;
      }
    }
  });
}
