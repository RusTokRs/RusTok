# rustok-ai-order documentation

This support crate owns the `order_analytics` and `order_ops_assistant`
descriptor and generated-payload contracts. It does not own provider execution
or order persistence.

`rustok-ai` composes the handlers, while `rustok-order` provides order-status
data through `CheckoutCompletionPort`. The live integration priorities are in
the [implementation plan](./implementation-plan.md).
