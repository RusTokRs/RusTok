export type ShippingProfileTranslation = {
  locale: string;
  name: string;
  description: string | null;
};

export type ShippingProfile = {
  id: string;
  slug: string;
  active: boolean;
  metadata: string;
  createdAt: string | null;
  updatedAt: string | null;
  translations: ShippingProfileTranslation[];
};

export type ShippingProfileList = {
  items: ShippingProfile[];
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
};

export type CreateShippingProfileInput = {
  slug: string;
  translations: ShippingProfileTranslation[];
  metadata?: string | null;
};

export type UpdateShippingProfileInput = {
  slug?: string | null;
  translations?: ShippingProfileTranslation[] | null;
  metadata?: string | null;
};

export type CartPromotionPreview = {
  kind: string;
  scope: string;
  lineItemId: string | null;
  currencyCode: string;
  baseAmount: string;
  adjustmentAmount: string;
  adjustedAmount: string;
};

export type CartAdjustment = {
  id: string;
  source: string | null;
  scope: string;
  lineItemId: string | null;
  amount: string;
  metadata: string;
};

export type CartSnapshot = {
  id: string;
  currencyCode: string;
  shippingTotal: string;
  adjustmentTotal: string;
  totalAmount: string;
  adjustments: CartAdjustment[];
};

export type AdminCartPromotionInput = {
  sourceId: string;
  kind: string;
  scope: string;
  lineItemId?: string | null;
  discountPercent?: string | null;
  amount?: string | null;
  metadata?: string | null;
};

export type OrderChange = {
  id: string;
  orderId: string;
  changeType: string;
  status: string;
  description: string | null;
  preview: string;
  metadata: string;
  createdAt: string | null;
  updatedAt: string | null;
};

export type OrderChangeList = {
  items: OrderChange[];
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
};

export type ExchangeDifferenceRefundInput = {
  amount: string;
  reason?: string | null;
  metadata?: string | null;
};

type OrderReturnSummary = {
  id: string;
  orderId: string;
  status: string;
  resolutionType: string | null;
  refundId: string | null;
  orderChangeId: string | null;
  metadata: string;
  createdAt: string | null;
  updatedAt: string | null;
  completedAt: string | null;
  cancelledAt: string | null;
};

type RefundSummary = {
  id: string;
  status: string;
  currencyCode: string;
  amount: string;
  reason: string | null;
  metadata: string;
  createdAt: string | null;
  updatedAt: string | null;
};

type CreateReturnDecisionItemInput = {
  lineItemId: string;
  quantity: number;
  reason?: string | null;
  note?: string | null;
  metadata?: string | null;
};

export type CreateReturnDecisionInput = {
  returnRequest: {
    reason?: string | null;
    note?: string | null;
    items?: CreateReturnDecisionItemInput[] | null;
    metadata?: string | null;
  };
  decision: {
    action: string;
    refund?: {
      paymentCollectionId?: string | null;
      amount?: string | null;
      reason?: string | null;
      metadata?: string | null;
    } | null;
    exchange?: {
      description?: string | null;
      preview?: string | null;
      metadata?: string | null;
    } | null;
    claim?: {
      description?: string | null;
      preview?: string | null;
      metadata?: string | null;
    } | null;
    metadata?: string | null;
  };
};

export type ReturnDecisionResponse = {
  action: string;
  orderReturn: OrderReturnSummary;
  refund: RefundSummary | null;
  orderChange: OrderChange | null;
  metadata: string;
};

export type OrderLineItem = {
  id: string;
  orderId: string;
  productId: string;
  variantId: string;
  shippingProfileSlug: string | null;
  sku: string | null;
  title: string;
  quantity: number;
  unitPrice: string;
  totalPrice: string;
  currencyCode: string;
  createdAt: string;
};

export type OrderListItem = {
  id: string;
  customerId: string;
  status: string;
  currencyCode: string;
  totalAmount: string;
  trackingNumber: string | null;
  carrier: string | null;
  createdAt: string;
  confirmedAt: string | null;
  paidAt: string | null;
  shippedAt: string | null;
  deliveredAt: string | null;
  cancelledAt: string | null;
  lineItems: OrderLineItem[];
};

export type OrderList = {
  items: OrderListItem[];
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
};

export type OrdersFilter = {
  status?: string;
  page?: number;
  perPage?: number;
};

export type AdminOrderLineItem = {
  id: string;
  orderId: string;
  productId: string | null;
  variantId: string | null;
  fulfillmentRequirement: string;
  shippingProfileSlug: string | null;
  sellerId: string | null;
  sku: string | null;
  title: string;
  quantity: number;
  unitPrice: string;
  totalPrice: string;
  currencyCode: string;
  createdAt: string;
};

export type AdminOrderAdjustment = {
  id: string;
  orderId: string;
  lineItemId: string | null;
  sourceType: string;
  sourceId: string | null;
  amount: string;
  currencyCode: string;
  createdAt: string;
};

export type AdminOrderTaxLine = {
  id: string;
  orderId: string;
  lineItemId: string | null;
  shippingOptionId: string | null;
  description: string | null;
  providerId: string;
  rate: string;
  amount: string;
  currencyCode: string;
  createdAt: string;
  updatedAt: string;
};

export type AdminOrder = {
  id: string;
  tenantId: string;
  channelId: string | null;
  channelSlug: string | null;
  customerId: string | null;
  status: string;
  currencyCode: string;
  subtotalAmount: string;
  adjustmentTotal: string;
  shippingTotal: string;
  totalAmount: string;
  taxTotal: string;
  taxIncluded: boolean;
  paymentId: string | null;
  paymentMethod: string | null;
  trackingNumber: string | null;
  carrier: string | null;
  cancellationReason: string | null;
  deliveredSignature: string | null;
  createdAt: string;
  updatedAt: string;
  confirmedAt: string | null;
  paidAt: string | null;
  shippedAt: string | null;
  deliveredAt: string | null;
  cancelledAt: string | null;
  lineItems: AdminOrderLineItem[];
  adjustments: AdminOrderAdjustment[];
  taxLines: AdminOrderTaxLine[];
};

export type AdminPayment = {
  id: string;
  paymentCollectionId: string;
  providerId: string;
  providerPaymentId: string;
  status: string;
  currencyCode: string;
  amount: string;
  capturedAmount: string;
  errorMessage: string | null;
  createdAt: string;
  updatedAt: string;
  authorizedAt: string | null;
  capturedAt: string | null;
  cancelledAt: string | null;
};

export type AdminRefund = {
  id: string;
  tenantId: string;
  paymentCollectionId: string;
  status: string;
  currencyCode: string;
  amount: string;
  reason: string | null;
  createdAt: string;
  updatedAt: string;
  refundedAt: string | null;
  cancelledAt: string | null;
};

export type AdminPaymentCollection = {
  id: string;
  tenantId: string;
  cartId: string | null;
  orderId: string | null;
  customerId: string | null;
  status: string;
  currencyCode: string;
  amount: string;
  authorizedAmount: string;
  capturedAmount: string;
  refundedAmount: string;
  providerId: string | null;
  cancellationReason: string | null;
  createdAt: string;
  updatedAt: string;
  authorizedAt: string | null;
  capturedAt: string | null;
  cancelledAt: string | null;
  payments: AdminPayment[];
  refunds: AdminRefund[];
};

export type AdminFulfillmentItem = {
  id: string;
  fulfillmentId: string;
  orderLineItemId: string;
  quantity: number;
  shippedQuantity: number;
  deliveredQuantity: number;
  createdAt: string;
  updatedAt: string;
};

export type AdminFulfillment = {
  id: string;
  tenantId: string;
  orderId: string;
  shippingOptionId: string | null;
  customerId: string | null;
  status: string;
  carrier: string | null;
  trackingNumber: string | null;
  deliveredNote: string | null;
  cancellationReason: string | null;
  items: AdminFulfillmentItem[];
  createdAt: string;
  updatedAt: string;
  shippedAt: string | null;
  deliveredAt: string | null;
  cancelledAt: string | null;
};

export type AdminOrderDetail = {
  order: AdminOrder;
  paymentCollection: AdminPaymentCollection | null;
  fulfillment: AdminFulfillment | null;
};

export type PaymentCollectionsFilter = {
  status?: string;
  orderId?: string;
  cartId?: string;
  customerId?: string;
  page?: number;
  perPage?: number;
};

export type PaymentCollectionList = {
  items: AdminPaymentCollection[];
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
};

export type RefundsFilter = {
  paymentCollectionId?: string;
  orderId?: string;
  status?: string;
  page?: number;
  perPage?: number;
};

export type RefundList = {
  items: AdminRefund[];
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
};

export type FulfillmentsFilter = {
  status?: string;
  orderId?: string;
  customerId?: string;
  page?: number;
  perPage?: number;
};

export type FulfillmentList = {
  items: AdminFulfillment[];
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
};
