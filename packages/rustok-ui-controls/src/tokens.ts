// GENERATED CODE - DO NOT MODIFY BY HAND
// Generated from UI/tokens/tokens.json
// Part of RusToK Design System (FFA Compatible)

export const RUSTOK_RADIUS_PX = {
  sm: 6,
  md: 10,
  lg: 14,
} as const;

export const RUSTOK_SPACING_PX = {
  '1': 4,
  '2': 8,
  '3': 12,
  '4': 16,
  '5': 20,
  '6': 24,
} as const;

export const RUSTOK_COLORS = {
  light: {
    bg: '#ffffff',
    fg: '#0f172a',
    muted: '#f1f5f9',
    mutedFg: '#64748b',
    border: '#e2e8f0',
    primary: '#0f172a',
    primaryFg: '#f8fafc',
    accent: '#f1f5f9',
    accentFg: '#0f172a',
    danger: '#ef4444',
    dangerFg: '#f8fafc',
  },
  dark: {
    bg: '#0f172a',
    fg: '#f8fafc',
    muted: '#1e293b',
    mutedFg: '#94a3b8',
    border: '#1e293b',
    primary: '#f8fafc',
    primaryFg: '#0f172a',
    accent: '#1e293b',
    accentFg: '#f8fafc',
    danger: '#7f1d1d',
    dangerFg: '#f8fafc',
  },
} as const;

export type RusTokRadiusKey = keyof typeof RUSTOK_RADIUS_PX;
export type RusTokSpacingKey = keyof typeof RUSTOK_SPACING_PX;
export type RusTokTheme = keyof typeof RUSTOK_COLORS;
