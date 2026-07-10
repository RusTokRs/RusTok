# rustok-ai-product documentation

This support crate owns the `product_copy` and `product_attributes` descriptor
and generated-payload contracts. It does not own provider execution or product
persistence.

`rustok-ai` composes the handlers, while `rustok-product` supplies catalog
context through `ProductCatalogReadPort`. Current integration priorities are in
the [implementation plan](./implementation-plan.md).
