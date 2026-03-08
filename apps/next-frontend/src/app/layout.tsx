import "@/styles/globals.css";
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";

import { EnabledModulesProvider } from "./providers/enabled-modules-provider";

export const metadata: Metadata = {
  title: "RusToK Storefront",
  description: "Next.js storefront for RusToK",
};

export default async function RootLayout({
  children,
}: {
  children: ReactNode;
}) {
  const locale = await getLocale();
  const messages = await getMessages();

  return (
    <html lang={locale}>
      <body className="min-h-screen bg-background text-foreground">
        <NextIntlClientProvider messages={messages}>
          <EnabledModulesProvider>{children}</EnabledModulesProvider>
        </NextIntlClientProvider>
      </body>
    </html>
  );
}

