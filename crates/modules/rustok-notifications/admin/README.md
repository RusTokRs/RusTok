# `rustok-notifications-admin`

Module-owned Leptos admin package for notification operations.

The foundation slice exposes an explicit source-registry status only. It does
not query persistence, synthesize delivery metrics, or keep host-owned shadow
state. Inbox, preference, fan-out, delivery-attempt, replay, and reconciliation
screens are added only after their owner APIs exist.

Public entry point: `NotificationsAdmin`.
