import "@/styles/globals.css";
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { FluentProvider } from "@rustok/next-fluent";
import { getLocale, getMessages } from "@rustok/next-fluent/server";
import "../i18n";

import { buildSeoMetadata } from "@/shared/seo/metadata";

import { EnabledModulesProvider } from "./providers/enabled-modules-provider";

export const metadata: Metadata = buildSeoMetadata();

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
        <FluentProvider locale={locale} messages={messages}>
          <EnabledModulesProvider>{children}</EnabledModulesProvider>
        </FluentProvider>
      </body>
    </html>
  );
}
