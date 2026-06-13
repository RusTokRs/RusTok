#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentCollectionCardData {
    pub id: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentCollectionCardLabels {
    pub badge: String,
    pub module_ownership: String,
    pub empty_id: String,
    pub empty_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentCollectionCardViewModel {
    pub collection_id: String,
    pub status: String,
}

pub fn build_payment_collection_card_view_model(
    payment_collection: Option<PaymentCollectionCardData>,
    labels: &PaymentCollectionCardLabels,
) -> PaymentCollectionCardViewModel {
    let (collection_id, status) = payment_collection
        .map(|collection| (collection.id, collection.status))
        .unwrap_or_else(|| (labels.empty_id.clone(), labels.empty_status.clone()));

    PaymentCollectionCardViewModel {
        collection_id,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> PaymentCollectionCardLabels {
        PaymentCollectionCardLabels {
            badge: "payment collection".into(),
            module_ownership: "Payment collection details stay in payment-owned UI".into(),
            empty_id: "not attached".into(),
            empty_status: "pending".into(),
        }
    }

    #[test]
    fn preserves_resolved_payment_collection() {
        let view_model = build_payment_collection_card_view_model(
            Some(PaymentCollectionCardData {
                id: "paycol_1".into(),
                status: "authorized".into(),
            }),
            &labels(),
        );

        assert_eq!(view_model.collection_id, "paycol_1");
        assert_eq!(view_model.status, "authorized");
    }

    #[test]
    fn applies_empty_payment_collection_fallback() {
        let view_model = build_payment_collection_card_view_model(None, &labels());

        assert_eq!(view_model.collection_id, "not attached");
        assert_eq!(view_model.status, "pending");
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentCollectionActionLabels {
    pub pending: String,
    pub create_or_reuse: String,
}

pub fn payment_collection_action_label(
    busy: bool,
    labels: &PaymentCollectionActionLabels,
) -> String {
    if busy {
        labels.pending.clone()
    } else {
        labels.create_or_reuse.clone()
    }
}
