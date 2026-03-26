import localFont from 'next/font/local';
import { GeistSans } from 'geist/font/sans';
import { GeistMono } from 'geist/font/mono';

import { cn } from '@/shared/lib/utils';

// Local fonts via @fontsource packages to avoid Google Fonts network dependency at build time.

const fontInstrument = localFont({
  src: '../../../../node_modules/@fontsource-variable/instrument-sans/files/instrument-sans-latin-wght-normal.woff2',
  variable: '--font-instrument',
  display: 'swap',
  weight: '400 700',
});

const fontNotoMono = localFont({
  src: '../../../../node_modules/@fontsource-variable/noto-sans-mono/files/noto-sans-mono-latin-wght-normal.woff2',
  variable: '--font-noto-mono',
  display: 'swap',
  weight: '100 900',
});

const fontMullish = localFont({
  src: '../../../../node_modules/@fontsource-variable/mulish/files/mulish-latin-wght-normal.woff2',
  variable: '--font-mullish',
  display: 'swap',
  weight: '200 1000',
});

const fontInter = localFont({
  src: '../../../../node_modules/@fontsource-variable/inter/files/inter-latin-wght-normal.woff2',
  variable: '--font-inter',
  display: 'swap',
  weight: '100 900',
});

const fontArchitectsDaughter = localFont({
  src: '../../../../node_modules/@fontsource/architects-daughter/files/architects-daughter-latin-400-normal.woff2',
  variable: '--font-architects-daughter',
  display: 'swap',
  weight: '400',
});

const fontDMSans = localFont({
  src: '../../../../node_modules/@fontsource-variable/dm-sans/files/dm-sans-latin-wght-normal.woff2',
  variable: '--font-dm-sans',
  display: 'swap',
  weight: '100 1000',
});

const fontFiraCode = localFont({
  src: '../../../../node_modules/@fontsource-variable/fira-code/files/fira-code-latin-wght-normal.woff2',
  variable: '--font-fira-code',
  display: 'swap',
  weight: '300 700',
});

const fontOutfit = localFont({
  src: '../../../../node_modules/@fontsource-variable/outfit/files/outfit-latin-wght-normal.woff2',
  variable: '--font-outfit',
  display: 'swap',
  weight: '100 900',
});

const fontSpaceMono = localFont({
  src: [
    {
      path: '../../../../node_modules/@fontsource/space-mono/files/space-mono-latin-400-normal.woff2',
      weight: '400',
      style: 'normal',
    },
    {
      path: '../../../../node_modules/@fontsource/space-mono/files/space-mono-latin-700-normal.woff2',
      weight: '700',
      style: 'normal',
    },
  ],
  variable: '--font-space-mono',
  display: 'swap',
});

export const fontVariables = cn(
  GeistSans.variable,
  GeistMono.variable,
  fontInstrument.variable,
  fontNotoMono.variable,
  fontMullish.variable,
  fontInter.variable,
  fontArchitectsDaughter.variable,
  fontDMSans.variable,
  fontFiraCode.variable,
  fontOutfit.variable,
  fontSpaceMono.variable
);
