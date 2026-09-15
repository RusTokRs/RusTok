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

### 5. Middleware (`middleware.ts`)

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
