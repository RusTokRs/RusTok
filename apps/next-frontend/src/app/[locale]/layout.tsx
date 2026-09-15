import type { ReactNode } from "react";
import { setRequestLocale } from "@rustok/next-fluent/server";

export default async function LocaleLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;

  setRequestLocale(locale);

  return children;
}
