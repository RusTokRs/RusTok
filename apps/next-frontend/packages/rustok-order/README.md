# @rustok/order-frontend

Frontend module package for Storefront Order Tracking and Customer Order History in RusToK Next.js.

## Overview

`@rustok/order-frontend` provides client-side order lookup, live lifecycle tracking, and customer order history components:
- **`OrderView`**: Full-page order details and 5-stage lifecycle progress tracker (*Placed → Paid → Processing → Shipped → Delivered*), tracking code copier, financial summaries, line items breakdown, recipient address card, and printable receipt.
- **`OrdersHistoryView`**: Customer portal view listing active and past orders, tab filters (*All, In Progress, Delivered, Cancelled*), quick order search, and order ID lookup for guest shoppers.

## GraphQL Contract

The package talks to backend GraphQL endpoints provided by `rustok-commerce`:
- **`storefrontOrder(id: UUID!)`**: Fetch single order by UUID.
- **`storefrontOrders(page: Int, perPage: Int, status: String)`**: Fetch paginated orders for the authenticated customer.

## Local Storage Support

For guest customers without an account, recent order IDs are securely remembered in `rustok_customer_recent_orders` and rendered in `OrdersHistoryView` with fast lookup.

## Usage

```tsx
import { OrderView, OrdersHistoryView, fetchStorefrontOrder } from "@rustok/order-frontend";

// Single order tracking view
export default async function OrderPage({ params }) {
  const { id, locale } = await params;
  const order = await fetchStorefrontOrder(storefrontGraphql, id);
  return <OrderView order={order} orderId={id} locale={locale} />;
}

// Order history list
export default function AccountOrdersPage({ locale, authToken }) {
  return <OrdersHistoryView locale={locale} authToken={authToken} />;
}
```
