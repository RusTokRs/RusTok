/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

// Workspace packages outside this app's node_modules ancestry still need React's JSX types.
// This is type-only: do not alias the real `react/jsx-runtime` import to a .d.ts file in
// tsconfig paths, or Turbopack can emit an undefined function in the browser bundle. The
// unaliased runtime must resolve the React package's own conditional exports instead.
declare module "react/jsx-runtime" {
  export * from "@rustok/react-jsx-runtime-types";
}
