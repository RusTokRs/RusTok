/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

// Typechecked with the app: the workspace bridge must retain React's real JSX types, not `any`.
type Assert<T extends true> = T;
type JsxKey = Parameters<typeof import('react/jsx-runtime').jsx>[2];
export type JsxKeyIsNotAny = Assert<0 extends (1 & JsxKey) ? false : true>;
export type JsxKeyRejectsBoolean = Assert<boolean extends JsxKey ? false : true>;
