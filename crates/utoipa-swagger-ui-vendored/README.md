# utoipa-swagger-ui-vendored

## Purpose
`crates/utoipa-swagger-ui-vendored` — module/application of RusToK. This is where its code and root documentation live.

## Interactions
- apps/server
- OpenAPI/Swagger UI endpoints
- tooling for API documentation

## Documentation
- Local documentation: `./docs/`
- General platform documentation: `/docs`

## Component passport
- **Role in the system:** Vendored Swagger UI crate for publishing OpenAPI API documentation.
- **Main data/responsibility:** business logic and API of this component; code and documentation structure at the component root.
- **Interacts with:**
  - apps/server
  - utoipa/OpenAPI pipeline
  - dev/documentation tools
- **Entry points:**
  - `crates/utoipa-swagger-ui-vendored/src/lib.rs`
- **Local documentation:** `./docs/`
- **Global platform documentation:** `/docs/`
