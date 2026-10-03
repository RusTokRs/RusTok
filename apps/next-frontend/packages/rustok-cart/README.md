# @rustok/cart-frontend

Frontend module package for the Storefront Shopping Cart in RusToK Next.js.

## Overview

`@rustok/cart-frontend` provides client-side cart management and presentation components:
- **`CartProvider` & `useCart()`**: Reactive context managing client cart state, local persistence (`rustok_storefront_cart_id`), and GraphQL mutations.
- **`CartDrawer`**: Slide-over drawer presenting the active line items, quantity steppers, price summaries (subtotal, adjustments, shipping, total), and checkout actions.
- **`CartTrigger`**: Floating or inline button displaying total items count with animated badge and click-to-open action.

## GraphQL Contract

The package talks to backend GraphQL endpoints provided by `rustok-commerce`:
- **`storefrontCart(id: UUID!)`**: Fetch existing cart by ID.
- **`createStorefrontCart(input: CreateStorefrontCartInput!)`**: Create a new session cart.
- **`addStorefrontCartLineItem(cartId: UUID!, input: AddStorefrontCartLineItemInput!)`**: Add variant to cart.
- **`updateStorefrontCartLineItemQuantity(cartId: UUID!, lineItemId: UUID!, quantity: Int!)`**: Update item count.
- **`removeStorefrontCartLineItem(cartId: UUID!, lineItemId: UUID!)`**: Remove item from cart.

## Usage

```tsx
import { CartProvider, CartDrawer, CartTrigger } from "@rustok/cart-frontend";

export default function Layout({ children, locale }) {
  return (
    <CartProvider locale={locale}>
      {children}
      <CartTrigger locale={locale} />
      <CartDrawer locale={locale} />
    </CartProvider>
  );
}
```
