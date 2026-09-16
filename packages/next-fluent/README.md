# @rustok/next-fluent

A modern, fast, lightweight localization library for Next.js App Router (React Server Components + Client Components) powered by Mozilla Project Fluent (`.ftl`).

## Features

- **Project Fluent Engine**: Full support for Mozilla Fluent syntax, advanced pluralization (`one`/`few`/`many`), terms, selectors, and variables via official `@fluent/bundle`.
- **Zero-Isolating Strings**: Renders clean strings without Unicode Bidirectional Isolating characters (FSI/PDI).
- **RSC Native**: `getTranslations(namespace?)` and `getLocale()` with request-level memoization via React `cache()`.
- **Client Components**: `<FluentProvider>` context and lightweight `useTranslations(namespace?)`, `useLocale()` hooks.
- **Middleware**: Built-in `createI18nMiddleware` for App Router URL prefixing, cookie management, and `Accept-Language` detection.
- **List / Attribute Support**: `t.raw(key)` formats message attributes (e.g. `.item0`, `.item1`) into arrays.
- **Zero Heavy Dependencies**: Pure TypeScript, minimal footprint.

## Installation

```bash
npm install @rustok/next-fluent
```

## Quick Start

### 1. Server Configuration (`src/i18n/request.ts`)

```typescript
import { setRequestConfig } from '@rustok/next-fluent/server';
import fs from 'node:fs/promises';
import path from 'node:path';

export default setRequestConfig(async ({ locale }) => {
  const resolvedLocale = locale ?? 'en';
  const filePath = path.join(process.cwd(), 'messages', `${resolvedLocale}.ftl`);
  const messages = await fs.readFile(filePath, 'utf-8');

  return {
    locale: resolvedLocale,
    messages,
  };
});
```

### 2. Root Layout (`app/layout.tsx`)

```tsx
import { FluentProvider } from '@rustok/next-fluent';
import { getLocale, getMessages } from '@rustok/next-fluent/server';

export default async function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const locale = await getLocale();
  const messages = await getMessages();

  return (
    <html lang={locale}>
      <body>
        <FluentProvider locale={locale} messages={messages}>
          {children}
        </FluentProvider>
      </body>
    </html>
  );
}
```

### 3. Server Components (RSC)

```tsx
import { getTranslations } from '@rustok/next-fluent/server';

export default async function Page() {
  const t = await getTranslations('Storefront');

  return (
    <div>
      <h1>{t('title')}</h1>
      <p>{t('welcome', { name: 'Alice' })}</p>
    </div>
  );
}
```

### 4. Client Components

```tsx
'use client';

import { useTranslations, useLocale } from '@rustok/next-fluent';

export function Navigation() {
  const t = useTranslations('app.nav');
  const locale = useLocale();

  return (
    <nav>
      <span>Current locale: {locale}</span>
      <a href="/">{t('dashboard')}</a>
    </nav>
  );
}
```

### 5. Rich Text & Interactive Markup (`t.rich`)

Format messages with interactive React elements or styled tags:

```ftl
terms-notice = By signing up you agree to our <terms>Terms of Service</terms> and <privacy>Privacy Policy</privacy>.
```

```tsx
const content = t.rich('terms-notice', {
  terms: (chunks) => <a href="/terms" className="underline font-semibold">{chunks}</a>,
  privacy: (chunks) => <a href="/privacy" className="underline font-semibold">{chunks}</a>,
});
```

### 6. Checking Key Existence (`t.has`)

```tsx
if (t.has('banner.promotion')) {
  return <PromoBanner message={t('banner.promotion')} />;
}
```

### 7. Type Safety & Code Generation (`next-fluent typegen`)

Generate TypeScript definitions directly from your `.ftl` catalogs for full autocompletion in your IDE:

```bash
npx next-fluent typegen --input messages/en.ftl --output src/types/i18n.d.ts
```

### 8. Fallback Locales & Modular Catalogs

Prevent missing translation keys by specifying a fallback bundle:

```tsx
<FluentProvider
  locale="ru"
  messages={ruCatalog}
  fallbackLocale="en"
  fallbackMessages={enCatalog}
>
  {children}
</FluentProvider>
```

Compose modular domain catalogs seamlessly:

```tsx
const messages = [baseCatalogFtl, blogModuleFtl, forumModuleFtl];
<FluentProvider locale={locale} messages={messages}>
  {children}
</FluentProvider>
```

### 9. Built-in Intl Functions (`CURRENCY` and `PERCENT`)

Format monetary amounts and percentages directly inside FTL without ad-hoc component code:

```ftl
cart-total = Total: { CURRENCY($total, currency: "USD") }
order-discount = Discount: { PERCENT($rate, minimumFractionDigits: 1) }
```

### 10. Debug Mode

Highlight missing translations in development:

```tsx
<FluentProvider locale={locale} messages={messages} debug={process.env.NODE_ENV !== 'production'}>
  {children}
</FluentProvider>
```

Missing keys return `[MISSING: key.name]` and output warnings to the developer console.

### 11. Pseudo-localization for UI Testing

Stress-test layout overflow, hardcoded dimensions, and text truncation using pseudo-localization:

```bash
npx next-fluent pseudo --input messages/en.ftl --output messages/en-XA.ftl
```

Produces accented, elongated text (`[Šţööŕééƒŕööñţ...]`) while preserving variables, tags, and selectors.

### 12. Direct `.ftl` Asset Imports (Docker & Edge)

Enable bundling `.ftl` catalogs directly into server JS bundles for standalone Docker / Vercel Edge / AWS Lambda:

```javascript
// next.config.mjs
export default {
  webpack(config) {
    config.module.rules.push({
      test: /\.ftl$/,
      type: 'asset/source',
    });
    return config;
  },
};
```

Import `.ftl` files directly:

```typescript
import enMessages from '@/messages/en.ftl';
```

### 13. Middleware (`middleware.ts`)

```typescript
import { createI18nMiddleware } from '@rustok/next-fluent/middleware';

export default createI18nMiddleware({
  locales: ['en', 'ru'],
  defaultLocale: 'en',
});

export const config = {
  matcher: ['/', '/((?!api|_next|_vercel|.*\\..*).*)'],
};
```

## Fluent `.ftl` Catalog Example

`messages/en.ftl`:
```ftl
Storefront-title = Welcome to RusToK Storefront
Storefront-welcome = Welcome, { $name }!
Storefront-cartItems = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
Storefront-features =
    .item0 = SSR + SEO out of the box
    .item1 = Fast access to GraphQL API
```

`messages/ru.ftl`:
```ftl
Storefront-title = Добро пожаловать на витрину RusToK
Storefront-welcome = Добро пожаловать, { $name }!
Storefront-cartItems = { $count ->
    [one] { $count } товар
    [few] { $count } товара
   *[other] { $count } товаров
}
Storefront-features =
    .item0 = SSR + SEO из коробки
    .item1 = Быстрый доступ к GraphQL API
```

## License

Business Source License 1.1 with RusToK Additional Use Grant.


