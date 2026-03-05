import nextPlugin from "@next/eslint-plugin-next";
import tsParser from "@typescript-eslint/parser";

const nextCoreWebVitalsRules = nextPlugin.configs["core-web-vitals"].rules;

export default [
  {
    ignores: [".next/**", "node_modules/**"],
  },
  {
    files: ["**/*.{js,jsx,ts,tsx}"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: {
      "@next/next": nextPlugin,
    },
    rules: {
      ...nextCoreWebVitalsRules,
    },
  },
];
