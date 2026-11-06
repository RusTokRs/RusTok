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
    ignores: [".next/", "out/", "build/", "node_modules/", "next-env.d.ts"],
  },
];
