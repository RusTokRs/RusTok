order-action-cancel = Cancel
order-action-deliver = Deliver
order-action-markPaid = Mark paid
order-action-open = Open
order-action-refresh = Refresh
order-action-ship = Ship
order-actionHint-cancelled = The order is cancelled; lifecycle buttons stay read-only.
order-actionHint-confirmed = The next operational step is marking the order as paid.
order-actionHint-delivered = The order is complete; only inspection remains.
order-actionHint-paid = The order is paid and ready for shipment.
order-actionHint-pending = This order is waiting for the next lifecycle event from checkout or operations.
order-actionHint-shipped = The order is in transit and can be marked as delivered.
order-badge = order
order-caption-created = created { $date }
order-caption-customer = customer { $customer }
order-detail-channel = channel { $channel }
order-detail-created = created { $date }
order-detail-empty = Open an order to inspect line items, payment state and fulfillment progress.
order-detail-none = No related record.
order-detail-subtitle = Read the operational snapshot and execute only the lifecycle step that matches the current state.
order-detail-title = Order detail
order-detail-updated = updated { $date }
order-error-bootstrapLoading = Bootstrap is still loading.
order-error-cancel = Failed to cancel order
order-error-deliver = Failed to deliver order
order-error-loadOrder = Failed to load order
order-error-loadOrders = Failed to load orders
order-error-markPaid = Failed to mark order as paid
order-error-markPaidRequirements = Payment id and payment method are required.
order-error-orderNotFound = Order not found.
order-error-selectionRequired = Open an order first.
order-error-ship = Failed to ship order
order-error-shipRequirements = Tracking number and carrier are required.
order-field-cancelReason = Cancellation reason
order-field-carrier = Carrier
order-field-deliveredSignature = Delivered signature
order-field-paymentId = Payment ID
order-field-paymentMethod = Payment method
order-field-trackingNumber = Tracking number
order-filter-allStatuses = All statuses
order-fulfillment-carrier = carrier: { $carrier }
order-fulfillment-deliveredNote = delivered note: { $note }
order-fulfillment-status = status: { $status }
order-fulfillment-tracking = tracking: { $tracking }
order-header-payment = payment { $id }
order-header-tracking = tracking { $tracking }
order-list-empty = No orders match the current filters.
order-list-subtitle = Inspect checkout-created orders and jump into operational state transitions.
order-list-title = Orders
order-lines-empty = no line items
order-lines-itemsCount =
    { $count ->
        [one] { $count } item
       *[other] { $count } items
    }
# plural-exempt: compact line item metadata
order-lines-lineDetails = { $sku } · qty { $quantity } · profile { $profile }
order-lines-more =
    { $count ->
        [one] +{ $count } more
       *[other] +{ $count } more
    }
order-lines-unitPrice = unit { $price }
order-loading = Loading...
order-payment-authorized = authorized: { $amount }
order-payment-captured = captured: { $amount }
order-payment-paymentsCount =
    { $count ->
        [one] payments: { $count }
       *[other] payments: { $count }
    }
order-payment-provider = provider: { $provider }
order-payment-status = status: { $status }
order-section-actions = Lifecycle actions
order-section-customer = Customer
order-section-fulfillment = Fulfillment
order-section-lifecycle = Lifecycle
order-section-lines = Line items
order-section-payment = Payment collection
order-status-cancelled = Cancelled
order-status-confirmed = Confirmed
order-status-delivered = Delivered
order-status-paid = Paid
order-status-pending = Pending
order-status-shipped = Shipped
order-subtitle = Module-owned operator workspace for order lifecycle, payment state visibility and delivery progress.
order-timeline-cancelled = cancelled { $date }
order-timeline-confirmed = confirmed { $date }
order-timeline-created = created { $date }
order-timeline-delivered = delivered { $date }
order-timeline-paid = paid { $date }
order-timeline-shipped = shipped { $date }
order-title = Order Operations

