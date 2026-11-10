import nextPlugin from "@next/eslint-plugin-next";
import nextParser from "eslint-config-next/parser";

export default [
  nextPlugin.configs["core-web-vitals"],
  {
    files: ["packages/rustok-blog/**/*.{ts,tsx}"],
    languageOptions: {
      parser: nextParser,
      parserOptions: {
        requireConfigFile: false,
        babelOptions: { presets: ["next/babel"] },
      },
    },
  },
  {
    // fly-actions.js is a byte-exact copy of the canonical Rust asset
    // (crates/ui/fly/browser/assets/fly-actions.js); lint/autofix would break
    // parity, which scripts/verify-fly-actions-parity.mjs enforces byte-for-byte.
    ignores: [
      ".next/",
      "out/",
      "build/",
      "node_modules/",
      "next-env.d.ts",
      "packages/rustok-pages/src/components/fly-actions.js",
    ],
  },
];
