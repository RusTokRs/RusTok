use async_trait::async_trait;
use rustok_api::PortError;
use rustok_events::{
    CHECKOUT_OPERATION_ADMISSION_CLOSED, CHECKOUT_OPERATION_ADMISSION_OPEN,
    CHECKOUT_OPERATION_ADMISSION_SETTLING,
};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::payment_collection;
use crate::error::PaymentResult;

/// Bounded provider execution admission level.
///
/// The vocabulary comes from `rustok-events`, the same schema the checkout
/// journal publishes in `checkout.operation.admission_changed`, so the level a
/// checkout writes and the level payment fences on cannot drift apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProviderExecutionAdmission {
    /// Extending provider execution is admitted for the current generation.
    Open,
    /// The checkout is compensating: extending effects are refused, unwinding
    /// effects stay admitted.
    Settling,
    /// The checkout reached a terminal status: no provider execution may extend
    /// the charge any more; unwinding effects stay admitted.
    Closed,
}

impl ProviderExecutionAdmission {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => CHECKOUT_OPERATION_ADMISSION_OPEN,
            Self::Settling => CHECKOUT_OPERATION_ADMISSION_SETTLING,
            Self::Closed => CHECKOUT_OPERATION_ADMISSION_CLOSED,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            CHECKOUT_OPERATION_ADMISSION_OPEN => Some(Self::Open),
            CHECKOUT_OPERATION_ADMISSION_SETTLING => Some(Self::Settling),
            CHECKOUT_OPERATION_ADMISSION_CLOSED => Some(Self::Closed),
            _ => None,
        }
    }
}

/// The admission generation payment observed on a checkout operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckoutExecutionAdmissionRecord {
    pub admission: ProviderExecutionAdmission,
    pub admission_epoch: i64,
}

/// Narrow owner port the checkout journal exposes to the payment claim gate.
///
/// Payment must not read `checkout_operations` directly: it asks the owner for
/// the two values the gate decides on. The port returns the *current* level and
/// generation, so payment never keeps a derived copy that could drift — the
/// failure mode the removed database trigger replaced by a hard coupling.
#[async_trait]
pub trait CheckoutExecutionAdmissionPort: Send + Sync {
    /// Reads the admission of one checkout operation.
    ///
    /// `Ok(None)` means the linked checkout operation does not exist (or is not
    /// visible to this tenant): the claim gate treats that as an unresolved
    /// admission and refuses extending effects instead of guessing.
    async fn read_checkout_execution_admission(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> Result<Option<CheckoutExecutionAdmissionRecord>, PortError>;
}

/// How a provider operation affects the money a checkout is holding.
///
/// *Extending* effects grow the charge (`authorize`, `capture`) and are fenced by
/// the admission level and generation. *Unwinding* effects (`cancel`, `refund`)
/// give money back and stay admitted while the checkout compensates — the
/// compensation itself needs the provider, which is exactly what the database
/// guard this contract replaces got wrong (ECOM-ADM-02).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderExecutionEffect {
    Extending,
    Unwinding,
}

impl ProviderExecutionEffect {
    /// Classifies a provider operation. `None` means the journal vocabulary is
    /// wider than this contract knows, which fails closed for a linked checkout.
    pub fn for_operation(operation: &str) -> Option<Self> {
        match operation {
            "authorize" | "capture" => Some(Self::Extending),
            "cancel" | "refund" => Some(Self::Unwinding),
            _ => None,
        }
    }

    /// Bounded metric label of the effect class.
    pub const fn refusal_metric_label(self) -> &'static str {
        match self {
            Self::Extending => "extending",
            Self::Unwinding => "unwinding",
        }
    }
}

/// Bounded `operation` label for the refusal metric.
///
/// The journal's `operation` column is written by the caller, so it must never
/// become a metric label verbatim: a value outside the four admitted operations
/// would create one time series per string, and the `EffectUnknown` case is by
/// definition outside that vocabulary. The label therefore carries the effect
/// class the gate decided on, and an operation the contract does not know
/// collapses into `unknown`, which is the vocabulary-drift signal an operator
/// can alert on. The raw string stays in the WARN log line next to the refusal.
pub fn refusal_metric_operation_label(operation: &str) -> &'static str {
    match ProviderExecutionEffect::for_operation(operation) {
        Some(effect) => effect.refusal_metric_label(),
        None => "unknown",
    }
}

/// Why the claim gate refused to start (or resume) provider execution.
///
/// The labels are the bounded `payment_provider_operations.admission_refusal_code`
/// vocabulary and the `reason` label of
/// `rustok_payment_provider_execution_admission_refused_total`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckoutAdmissionRefusal {
    /// The checkout is compensating: extending execution is refused.
    Settling,
    /// The checkout reached a terminal status: extending execution is refused.
    Closed,
    /// The linked checkout operation has no readable admission (missing row or a
    /// failing owner port); extending execution fails closed.
    Unavailable,
    /// The operation carries a generation the checkout has already left: an
    /// in-flight claim from an older incarnation must never execute.
    EpochMismatch,
    /// The operation string is outside the bounded journal vocabulary, so the
    /// effect cannot be classified.
    EffectUnknown,
}

impl CheckoutAdmissionRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Settling => "checkout_admission_settling",
            Self::Closed => "checkout_admission_closed",
            Self::Unavailable => "checkout_admission_unavailable",
            Self::EpochMismatch => "checkout_admission_epoch_mismatch",
            Self::EffectUnknown => "checkout_admission_effect_unknown",
        }
    }

    /// Every refusal, in contract order.
    pub const ALL: [Self; 5] = [
        Self::Settling,
        Self::Closed,
        Self::Unavailable,
        Self::EpochMismatch,
        Self::EffectUnknown,
    ];

    /// Parses a recorded refusal code. The code column is bounded by the
    /// migration's `CHECK` on PostgreSQL/MySQL and by the writer elsewhere. The
    /// vocabulary is read from [`Self::as_str`] so a new refusal cannot be added
    /// to one direction only.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|refusal| refusal.as_str() == value)
    }

    /// Bounded owner error code surfaced to the caller of the claim gate.
    pub const fn error_code(self) -> &'static str {
        match self {
            Self::Settling => "payment.checkout_admission_settling",
            Self::Closed => "payment.checkout_admission_closed",
            Self::Unavailable => "payment.checkout_admission_unavailable",
            Self::EpochMismatch => "payment.checkout_admission_epoch_mismatch",
            Self::EffectUnknown => "payment.checkout_admission_effect_unknown",
        }
    }
}

/// Projected admission of one checkout operation as payment sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckoutAdmissionLinkState {
    /// The payment collection carries no checkout operation: the claim is not
    /// fenced at all (parity with the database guard this contract replaces).
    Unlinked,
    /// The operation is linked and its admission was read from the owner.
    Resolved(CheckoutExecutionAdmissionRecord),
    /// The operation is linked but its admission is not readable (drift).
    Unavailable,
}

/// Outcome of the claim decision, computed by a pure function so the rule is
/// testable without a database and has exactly one implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckoutAdmissionDecision {
    /// No generation check is required: the collection is unlinked, or the claim
    /// is an unwinding effect, which must stay able to give money back.
    Unfenced,
    /// Admitted extending execution under `epoch`; `adopt_legacy` marks a row
    /// that predates the contract (generation `0`) and is stamped on success.
    Admitted { epoch: i64, adopt_legacy: bool },
    /// Refused; the caller records the bounded reason and increments the metric.
    Refused(CheckoutAdmissionRefusal),
}

/// Decides whether a provider operation may be claimed for execution.
///
/// * A row created before the admission contract (`stored_epoch == 0`) is
///   adopted into the generation the owner reports, but only while the level is
///   `open` — exactly the behaviour the database guard had for pre-existing rows.
/// * A stored generation the checkout has left means the operation belongs to an
///   older incarnation: it is refused instead of racing the park that replaced
///   it (`checkout_admission_epoch_mismatch`).
/// * Unwinding effects skip the fence entirely: `settling`, `closed`,
///   `Unavailable` and a stale generation must never be able to trap money that
///   has to move back.
pub fn decide_checkout_admission_claim(
    link: CheckoutAdmissionLinkState,
    effect: Option<ProviderExecutionEffect>,
    stored_epoch: i64,
) -> CheckoutAdmissionDecision {
    match effect {
        None => match link {
            // An unlinked collection is outside the contract, so an unknown
            // operation string is not a link violation.
            CheckoutAdmissionLinkState::Unlinked => CheckoutAdmissionDecision::Unfenced,
            _ => CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::EffectUnknown),
        },
        Some(ProviderExecutionEffect::Unwinding) => CheckoutAdmissionDecision::Unfenced,
        Some(ProviderExecutionEffect::Extending) => match link {
            CheckoutAdmissionLinkState::Unlinked => CheckoutAdmissionDecision::Unfenced,
            CheckoutAdmissionLinkState::Unavailable => {
                CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::Unavailable)
            }
            CheckoutAdmissionLinkState::Resolved(record) => match record.admission {
                ProviderExecutionAdmission::Settling => {
                    CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::Settling)
                }
                ProviderExecutionAdmission::Closed => {
                    CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::Closed)
                }
                ProviderExecutionAdmission::Open => {
                    if stored_epoch == 0 {
                        CheckoutAdmissionDecision::Admitted {
                            epoch: record.admission_epoch,
                            adopt_legacy: true,
                        }
                    } else if stored_epoch == record.admission_epoch {
                        CheckoutAdmissionDecision::Admitted {
                            epoch: record.admission_epoch,
                            adopt_legacy: false,
                        }
                    } else {
                        CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::EpochMismatch)
                    }
                }
            },
        },
    }
}

/// Checkout operation a payment collection is linked to, if any.
///
/// This is the same `payment_collections.metadata -> checkout.operation_id`
/// link the database guard read before the claim gate replaced it, so the two
/// implementations cannot disagree about which checkout fences a collection.
pub fn checkout_operation_id_from_metadata(metadata: &serde_json::Value) -> Option<Uuid> {
    metadata
        .get("checkout")
        .and_then(|checkout| checkout.get("operation_id"))
        .and_then(|operation_id| operation_id.as_str())
        .and_then(|operation_id| Uuid::parse_str(operation_id.trim()).ok())
}

/// Resolves the checkout operation a payment collection belongs to.
pub async fn resolve_checkout_operation_id<C>(
    db: &C,
    tenant_id: Uuid,
    payment_collection_id: Uuid,
) -> PaymentResult<Option<Uuid>>
where
    C: ConnectionTrait,
{
    let collection = payment_collection::Entity::find_by_id(payment_collection_id)
        .filter(payment_collection::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let Some(collection) = collection else {
        return Ok(None);
    };
    Ok(checkout_operation_id_from_metadata(&collection.metadata))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        admission: ProviderExecutionAdmission,
        admission_epoch: i64,
    ) -> CheckoutAdmissionLinkState {
        CheckoutAdmissionLinkState::Resolved(CheckoutExecutionAdmissionRecord {
            admission,
            admission_epoch,
        })
    }

    #[test]
    fn admission_level_vocabulary_matches_the_event_contract() {
        for level in [
            ProviderExecutionAdmission::Open,
            ProviderExecutionAdmission::Settling,
            ProviderExecutionAdmission::Closed,
        ] {
            assert_eq!(
                ProviderExecutionAdmission::parse(level.as_str()),
                Some(level)
            );
        }
        assert_eq!(ProviderExecutionAdmission::parse("frozen"), None);
    }

    #[test]
    fn effects_split_extending_from_unwinding_operations() {
        assert_eq!(
            ProviderExecutionEffect::for_operation("authorize"),
            Some(ProviderExecutionEffect::Extending)
        );
        assert_eq!(
            ProviderExecutionEffect::for_operation("capture"),
            Some(ProviderExecutionEffect::Extending)
        );
        assert_eq!(
            ProviderExecutionEffect::for_operation("cancel"),
            Some(ProviderExecutionEffect::Unwinding)
        );
        assert_eq!(
            ProviderExecutionEffect::for_operation("refund"),
            Some(ProviderExecutionEffect::Unwinding)
        );
        assert_eq!(ProviderExecutionEffect::for_operation("chargeback"), None);
    }

    #[test]
    fn unlinked_collections_are_not_fenced() {
        for effect in [
            Some(ProviderExecutionEffect::Extending),
            Some(ProviderExecutionEffect::Unwinding),
            None,
        ] {
            assert_eq!(
                decide_checkout_admission_claim(CheckoutAdmissionLinkState::Unlinked, effect, 0),
                CheckoutAdmissionDecision::Unfenced
            );
        }
    }

    #[test]
    fn extending_claims_are_fenced_by_level_and_generation() {
        assert_eq!(
            decide_checkout_admission_claim(
                record(ProviderExecutionAdmission::Open, 7),
                Some(ProviderExecutionEffect::Extending),
                7
            ),
            CheckoutAdmissionDecision::Admitted {
                epoch: 7,
                adopt_legacy: false
            }
        );
        assert_eq!(
            decide_checkout_admission_claim(
                record(ProviderExecutionAdmission::Open, 7),
                Some(ProviderExecutionEffect::Extending),
                0
            ),
            CheckoutAdmissionDecision::Admitted {
                epoch: 7,
                adopt_legacy: true
            }
        );
        assert_eq!(
            decide_checkout_admission_claim(
                record(ProviderExecutionAdmission::Open, 7),
                Some(ProviderExecutionEffect::Extending),
                6
            ),
            CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::EpochMismatch)
        );

        for (admission, refusal) in [
            (
                ProviderExecutionAdmission::Settling,
                CheckoutAdmissionRefusal::Settling,
            ),
            (
                ProviderExecutionAdmission::Closed,
                CheckoutAdmissionRefusal::Closed,
            ),
        ] {
            assert_eq!(
                decide_checkout_admission_claim(
                    record(admission, 7),
                    Some(ProviderExecutionEffect::Extending),
                    7
                ),
                CheckoutAdmissionDecision::Refused(refusal)
            );
        }
    }

    #[test]
    fn unwinding_effects_stay_admitted_while_the_checkout_settles() {
        for link in [
            record(ProviderExecutionAdmission::Settling, 3),
            record(ProviderExecutionAdmission::Closed, 4),
            CheckoutAdmissionLinkState::Unavailable,
            CheckoutAdmissionLinkState::Unlinked,
        ] {
            assert_eq!(
                decide_checkout_admission_claim(link, Some(ProviderExecutionEffect::Unwinding), 3),
                CheckoutAdmissionDecision::Unfenced
            );
        }
    }

    #[test]
    fn unreadable_admission_and_unknown_effects_fail_closed() {
        assert_eq!(
            decide_checkout_admission_claim(
                CheckoutAdmissionLinkState::Unavailable,
                Some(ProviderExecutionEffect::Extending),
                1
            ),
            CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::Unavailable)
        );
        assert_eq!(
            decide_checkout_admission_claim(CheckoutAdmissionLinkState::Unavailable, None, 1),
            CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::EffectUnknown)
        );
        assert_eq!(
            decide_checkout_admission_claim(record(ProviderExecutionAdmission::Open, 1), None, 1),
            CheckoutAdmissionDecision::Refused(CheckoutAdmissionRefusal::EffectUnknown)
        );
    }

    #[test]
    fn refusal_vocabulary_is_bounded() {
        for refusal in CheckoutAdmissionRefusal::ALL {
            assert!(refusal.as_str().starts_with("checkout_admission_"));
            assert!(
                refusal
                    .error_code()
                    .starts_with("payment.checkout_admission_")
            );
            assert_eq!(
                CheckoutAdmissionRefusal::parse(refusal.as_str()),
                Some(refusal)
            );
        }
        // The refusal codes the database `CHECK` and the metric label use are the
        // same five strings, in the same order.
        assert_eq!(
            CheckoutAdmissionRefusal::ALL.map(CheckoutAdmissionRefusal::as_str),
            [
                "checkout_admission_settling",
                "checkout_admission_closed",
                "checkout_admission_unavailable",
                "checkout_admission_epoch_mismatch",
                "checkout_admission_effect_unknown",
            ]
        );
    }

    #[test]
    fn checkout_link_is_read_from_the_collection_metadata() {
        let operation_id = Uuid::new_v4();
        assert_eq!(
            checkout_operation_id_from_metadata(&serde_json::json!({
                "checkout": { "operation_id": operation_id.to_string() }
            })),
            Some(operation_id)
        );
        assert_eq!(
            checkout_operation_id_from_metadata(&serde_json::json!({
                "checkout": { "operation_id": "not-a-uuid" }
            })),
            None
        );
        assert_eq!(
            checkout_operation_id_from_metadata(&serde_json::json!({})),
            None
        );
    }
}
