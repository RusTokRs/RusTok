use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::model::{
    BundleAdminCommand, BundleAdminCommandResult, BundleAdminDirectory, BundleAdminFilters,
    BundleAdminItemRecord, BundleAdminListItem, BundleAdminRecord,
};

pub type GraphqlBundleAdminError = String;

const DIRECTORY_QUERY: &str = r#"
query BundleAdminDirectory($page: Int, $perPage: Int, $search: String, $status: String, $bundleType: String) {
  bundles(filter: { page: $page, perPage: $perPage, search: $search, status: $status, bundleType: $bundleType }) {
    total
    page
    perPage
    items {
      id
      tenantId
      slug
      name
      description
      bundleType
      status
      discountType
      discountValue
      itemsCount
      createdAt
      updatedAt
    }
  }
}
"#;

const DETAIL_QUERY: &str = r#"
query BundleAdminDetail($id: UUID!) {
  bundle(id: $id) {
    id
    tenantId
    bundleProductId
    slug
    name
    description
    bundleType
    status
    discountType
    discountValue
    metadata
    createdAt
    updatedAt
    items {
      id
      bundleId
      productId
      variantId
      quantity
      isOptional
      discountRate
      position
    }
  }
}
"#;

const CREATE_MUTATION: &str = r#"
mutation BundleAdminCreate($input: CreateBundleInputGql!) {
  createBundle(input: $input) {
    id
    tenantId
    bundleProductId
    slug
    name
    description
    bundleType
    status
    discountType
    discountValue
    metadata
    createdAt
    updatedAt
  }
}
"#;

const UPDATE_MUTATION: &str = r#"
mutation BundleAdminUpdate($id: UUID!, $input: UpdateBundleInputGql!) {
  updateBundle(id: $id, input: $input) {
    id
    tenantId
    bundleProductId
    slug
    name
    description
    bundleType
    status
    discountType
    discountValue
    metadata
    createdAt
    updatedAt
  }
}
"#;

const DELETE_MUTATION: &str = r#"
mutation BundleAdminDelete($id: UUID!) {
  deleteBundle(id: $id)
}
"#;

const ADD_ITEM_MUTATION: &str = r#"
mutation BundleAdminAddItem($bundleId: UUID!, $input: AddBundleItemInputGql!) {
  addBundleItem(bundleId: $bundleId, input: $input) {
    id
    bundleId
    productId
    variantId
    quantity
    isOptional
    discountRate
    position
  }
}
"#;

const REMOVE_ITEM_MUTATION: &str = r#"
mutation BundleAdminRemoveItem($bundleId: UUID!, $itemId: UUID!) {
  removeBundleItem(bundleId: $bundleId, itemId: $itemId)
}
"#;

#[derive(Debug, Serialize)]
struct DirectoryVariables {
    page: i64,
    #[serde(rename = "perPage")]
    per_page: i64,
    search: Option<String>,
    status: Option<String>,
    #[serde(rename = "bundleType")]
    bundle_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    bundles: DirectoryWire,
}

#[derive(Debug, Deserialize)]
struct DirectoryWire {
    items: Vec<ListItemWire>,
    total: u64,
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

#[derive(Debug, Deserialize)]
struct ListItemWire {
    id: String,
    #[serde(rename = "tenantId")]
    tenant_id: String,
    slug: String,
    name: String,
    description: Option<String>,
    #[serde(rename = "bundleType")]
    bundle_type: String,
    status: String,
    #[serde(rename = "discountType")]
    discount_type: String,
    #[serde(rename = "discountValue")]
    discount_value: String,
    #[serde(rename = "itemsCount", default)]
    items_count: i64,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct DetailResponse {
    bundle: Option<RecordWire>,
}

#[derive(Debug, Deserialize)]
struct RecordWire {
    id: String,
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(rename = "bundleProductId")]
    bundle_product_id: Option<String>,
    slug: String,
    name: String,
    description: Option<String>,
    #[serde(rename = "bundleType")]
    bundle_type: String,
    status: String,
    #[serde(rename = "discountType")]
    discount_type: String,
    #[serde(rename = "discountValue")]
    discount_value: String,
    #[serde(default)]
    metadata: serde_json::Value,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    #[serde(default)]
    items: Vec<ItemWire>,
}

#[derive(Debug, Deserialize)]
struct ItemWire {
    id: String,
    #[serde(rename = "bundleId")]
    bundle_id: String,
    #[serde(rename = "productId")]
    product_id: String,
    #[serde(rename = "variantId")]
    variant_id: Option<String>,
    quantity: i32,
    #[serde(rename = "isOptional")]
    is_optional: bool,
    #[serde(rename = "discountRate")]
    discount_rate: Option<String>,
    position: i32,
}

#[derive(Debug, Deserialize)]
struct CreateResponse {
    #[serde(rename = "createBundle")]
    bundle: RecordWire,
}

#[derive(Debug, Deserialize)]
struct UpdateResponse {
    #[serde(rename = "updateBundle")]
    bundle: RecordWire,
}

#[derive(Debug, Deserialize)]
struct DeleteResponse {
    #[serde(rename = "deleteBundle")]
    success: bool,
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, GraphqlBundleAdminError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    "http://localhost:5150/api/graphql".to_string()
}

pub async fn load_directory(
    token: Option<String>,
    tenant_slug: Option<String>,
    filters: BundleAdminFilters,
) -> Result<BundleAdminDirectory, GraphqlBundleAdminError> {
    let page = filters.page.max(1);
    let per_page = filters.per_page.clamp(1, 100);

    let vars = DirectoryVariables {
        page: page as i64,
        per_page: per_page as i64,
        search: filters.search,
        status: filters.status,
        bundle_type: filters.bundle_type,
    };

    let data: DirectoryResponse = request(DIRECTORY_QUERY, vars, token, tenant_slug).await?;
    Ok(BundleAdminDirectory {
        items: data
            .bundles
            .items
            .into_iter()
            .map(|i| BundleAdminListItem {
                id: i.id,
                tenant_id: i.tenant_id,
                slug: i.slug,
                name: i.name,
                description: i.description,
                bundle_type: i.bundle_type,
                status: i.status,
                discount_type: i.discount_type,
                discount_value: i.discount_value,
                items_count: i.items_count,
                created_at: i.created_at,
                updated_at: i.updated_at,
            })
            .collect(),
        total: data.bundles.total,
        page: data.bundles.page,
        per_page: data.bundles.per_page,
    })
}

pub async fn load_detail(
    token: Option<String>,
    tenant_slug: Option<String>,
    bundle_id: String,
) -> Result<BundleAdminRecord, GraphqlBundleAdminError> {
    #[derive(Serialize)]
    struct Vars {
        id: String,
    }
    let data: DetailResponse = request(DETAIL_QUERY, Vars { id: bundle_id }, token, tenant_slug).await?;
    let record = data
        .bundle
        .ok_or_else(|| "Bundle not found".to_string())?;

    Ok(BundleAdminRecord {
        id: record.id,
        tenant_id: record.tenant_id,
        bundle_product_id: record.bundle_product_id,
        slug: record.slug,
        name: record.name,
        description: record.description,
        bundle_type: record.bundle_type,
        status: record.status,
        discount_type: record.discount_type,
        discount_value: record.discount_value,
        metadata: record.metadata,
        created_at: record.created_at,
        updated_at: record.updated_at,
        translations: Vec::new(),
        items: record
            .items
            .into_iter()
            .map(|i| BundleAdminItemRecord {
                id: i.id,
                bundle_id: i.bundle_id,
                product_id: i.product_id,
                variant_id: i.variant_id,
                quantity: i.quantity,
                is_optional: i.is_optional,
                discount_rate: i.discount_rate,
                position: i.position,
            })
            .collect(),
    })
}

pub async fn execute_command(
    token: Option<String>,
    tenant_slug: Option<String>,
    _idempotency_key: String,
    command: BundleAdminCommand,
) -> Result<BundleAdminCommandResult, GraphqlBundleAdminError> {
    match command {
        BundleAdminCommand::Create { draft } => {
            #[derive(Serialize)]
            struct Input {
                slug: String,
                name: String,
                description: Option<String>,
                #[serde(rename = "bundleType")]
                bundle_type: String,
                status: String,
                #[serde(rename = "discountType")]
                discount_type: String,
                #[serde(rename = "discountValue")]
                discount_value: String,
            }
            #[derive(Serialize)]
            struct Vars {
                input: Input,
            }

            let vars = Vars {
                input: Input {
                    slug: draft.slug,
                    name: draft.name,
                    description: draft.description,
                    bundle_type: draft.bundle_type,
                    status: draft.status,
                    discount_type: draft.discount_type,
                    discount_value: draft.discount_value,
                },
            };

            let data: CreateResponse = request(CREATE_MUTATION, vars, token, tenant_slug).await?;
            Ok(BundleAdminCommandResult {
                bundle: Some(BundleAdminRecord {
                    id: data.bundle.id,
                    tenant_id: data.bundle.tenant_id,
                    bundle_product_id: data.bundle.bundle_product_id,
                    slug: data.bundle.slug,
                    name: data.bundle.name,
                    description: data.bundle.description,
                    bundle_type: data.bundle.bundle_type,
                    status: data.bundle.status,
                    discount_type: data.bundle.discount_type,
                    discount_value: data.bundle.discount_value,
                    metadata: data.bundle.metadata,
                    created_at: data.bundle.created_at,
                    updated_at: data.bundle.updated_at,
                    translations: Vec::new(),
                    items: Vec::new(),
                }),
                success: true,
            })
        }
        BundleAdminCommand::Update { id, draft } => {
            #[derive(Serialize)]
            struct Input {
                slug: Option<String>,
                name: Option<String>,
                description: Option<String>,
                #[serde(rename = "bundleType")]
                bundle_type: Option<String>,
                status: Option<String>,
                #[serde(rename = "discountType")]
                discount_type: Option<String>,
                #[serde(rename = "discountValue")]
                discount_value: Option<String>,
            }
            #[derive(Serialize)]
            struct Vars {
                id: String,
                input: Input,
            }

            let vars = Vars {
                id,
                input: Input {
                    slug: draft.slug,
                    name: draft.name,
                    description: draft.description,
                    bundle_type: draft.bundle_type,
                    status: draft.status,
                    discount_type: draft.discount_type,
                    discount_value: draft.discount_value,
                },
            };

            let data: UpdateResponse = request(UPDATE_MUTATION, vars, token, tenant_slug).await?;
            Ok(BundleAdminCommandResult {
                bundle: Some(BundleAdminRecord {
                    id: data.bundle.id,
                    tenant_id: data.bundle.tenant_id,
                    bundle_product_id: data.bundle.bundle_product_id,
                    slug: data.bundle.slug,
                    name: data.bundle.name,
                    description: data.bundle.description,
                    bundle_type: data.bundle.bundle_type,
                    status: data.bundle.status,
                    discount_type: data.bundle.discount_type,
                    discount_value: data.bundle.discount_value,
                    metadata: data.bundle.metadata,
                    created_at: data.bundle.created_at,
                    updated_at: data.bundle.updated_at,
                    translations: Vec::new(),
                    items: Vec::new(),
                }),
                success: true,
            })
        }
        BundleAdminCommand::Delete { id } => {
            #[derive(Serialize)]
            struct Vars {
                id: String,
            }
            let data: DeleteResponse = request(DELETE_MUTATION, Vars { id }, token, tenant_slug).await?;
            Ok(BundleAdminCommandResult {
                bundle: None,
                success: data.success,
            })
        }
        BundleAdminCommand::AddItem {
            bundle_id,
            product_id,
            variant_id,
            quantity,
            is_optional,
        } => {
            #[derive(Serialize)]
            struct Input {
                #[serde(rename = "productId")]
                product_id: String,
                #[serde(rename = "variantId")]
                variant_id: Option<String>,
                quantity: i32,
                #[serde(rename = "isOptional")]
                is_optional: bool,
            }
            #[derive(Serialize)]
            struct Vars {
                #[serde(rename = "bundleId")]
                bundle_id: String,
                input: Input,
            }

            let vars = Vars {
                bundle_id: bundle_id.clone(),
                input: Input {
                    product_id,
                    variant_id,
                    quantity,
                    is_optional,
                },
            };

            let _: serde_json::Value = request(ADD_ITEM_MUTATION, vars, token.clone(), tenant_slug.clone()).await?;
            let record = load_detail(token, tenant_slug, bundle_id).await?;
            Ok(BundleAdminCommandResult {
                bundle: Some(record),
                success: true,
            })
        }
        BundleAdminCommand::RemoveItem {
            bundle_id,
            item_id,
        } => {
            #[derive(Serialize)]
            struct Vars {
                #[serde(rename = "bundleId")]
                bundle_id: String,
                #[serde(rename = "itemId")]
                item_id: String,
            }

            let vars = Vars {
                bundle_id: bundle_id.clone(),
                item_id,
            };

            let _: serde_json::Value = request(REMOVE_ITEM_MUTATION, vars, token.clone(), tenant_slug.clone()).await?;
            let record = load_detail(token, tenant_slug, bundle_id).await?;
            Ok(BundleAdminCommandResult {
                bundle: Some(record),
                success: true,
            })
        }
    }
}
