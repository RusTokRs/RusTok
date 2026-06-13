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
