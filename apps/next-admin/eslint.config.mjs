import nextPlugin from '@next/eslint-plugin-next';

export default [
  nextPlugin.configs['core-web-vitals'],
  {
    ignores: ['.next/', 'out/', 'build/', 'node_modules/', 'next-env.d.ts'],
  },
];
