use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::model::{
    BrandAdminCommand, BrandAdminCommandResult, BrandAdminDirectory, BrandAdminFilters,
    BrandAdminListItem, BrandAdminRecord,
};

pub type GraphqlBrandAdminError = String;

const DIRECTORY_QUERY: &str = r#"
query BrandAdminDirectory($page: Int, $perPage: Int, $search: String, $isActive: Boolean) {
  brands(filter: { page: $page, perPage: $perPage, search: $search, isActive: $isActive }) {
    total
    page
    perPage
    items {
      id
      tenantId
      slug
      name
      description
      websiteUrl
      logoUrl
      isActive
      sortOrder
      productsCount
      createdAt
      updatedAt
    }
  }
}
"#;

const DETAIL_QUERY: &str = r#"
query BrandAdminDetail($id: UUID!) {
  brand(id: $id) {
    id
    tenantId
    slug
    name
    description
    websiteUrl
    logoUrl
    metadata
    isActive
    sortOrder
    createdAt
    updatedAt
  }
}
"#;

const CREATE_MUTATION: &str = r#"
mutation BrandAdminCreate($input: CreateBrandInputGql!) {
  createBrand(input: $input) {
    id
    tenantId
    slug
    name
    description
    websiteUrl
    logoUrl
    metadata
    isActive
    sortOrder
    createdAt
    updatedAt
  }
}
"#;

const UPDATE_MUTATION: &str = r#"
mutation BrandAdminUpdate($id: UUID!, $input: UpdateBrandInputGql!) {
  updateBrand(id: $id, input: $input) {
    id
    tenantId
    slug
    name
    description
    websiteUrl
    logoUrl
    metadata
    isActive
    sortOrder
    createdAt
    updatedAt
  }
}
"#;

const DELETE_MUTATION: &str = r#"
mutation BrandAdminDelete($id: UUID!) {
  deleteBrand(id: $id)
}
"#;

#[derive(Debug, Serialize)]
struct DirectoryVariables {
    page: i64,
    #[serde(rename = "perPage")]
    per_page: i64,
    search: Option<String>,
    #[serde(rename = "isActive")]
    is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    brands: DirectoryWire,
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
    #[serde(rename = "websiteUrl")]
    website_url: Option<String>,
    #[serde(rename = "logoUrl")]
    logo_url: Option<String>,
    #[serde(rename = "isActive")]
    is_active: bool,
    #[serde(rename = "sortOrder")]
    sort_order: i32,
    #[serde(rename = "productsCount")]
    products_count: i64,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct DetailResponse {
    brand: Option<RecordWire>,
}

#[derive(Debug, Deserialize)]
struct RecordWire {
    id: String,
    #[serde(rename = "tenantId")]
    tenant_id: String,
    slug: String,
    name: String,
    description: Option<String>,
    #[serde(rename = "websiteUrl")]
    website_url: Option<String>,
    #[serde(rename = "logoUrl")]
    logo_url: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
    #[serde(rename = "isActive")]
    is_active: bool,
    #[serde(rename = "sortOrder")]
    sort_order: i32,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateResponse {
    #[serde(rename = "createBrand")]
    brand: RecordWire,
}

#[derive(Debug, Deserialize)]
struct UpdateResponse {
    #[serde(rename = "updateBrand")]
    brand: RecordWire,
}

#[derive(Debug, Deserialize)]
struct DeleteResponse {
    #[serde(rename = "deleteBrand")]
    success: bool,
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, GraphqlBrandAdminError>
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


pub async fn load_directory(
    access_token: Option<String>,
    tenant_slug: Option<String>,
    filters: BrandAdminFilters,
) -> Result<BrandAdminDirectory, GraphqlBrandAdminError> {
    let variables = DirectoryVariables {
        page: filters.page.max(1) as i64,
        per_page: filters.per_page.clamp(1, 100) as i64,
        search: filters.search,
        is_active: filters.is_active,
    };

    let response: DirectoryResponse =
        request(DIRECTORY_QUERY, variables, access_token, tenant_slug).await?;

    Ok(BrandAdminDirectory {
        items: response
            .brands
            .items
            .into_iter()
            .map(|item| BrandAdminListItem {
                id: item.id,
                tenant_id: item.tenant_id,
                slug: item.slug,
                name: item.name,
                description: item.description,
                website_url: item.website_url,
                logo_url: item.logo_url,
                is_active: item.is_active,
                sort_order: item.sort_order,
                products_count: item.products_count,
                created_at: item.created_at,
                updated_at: item.updated_at,
            })
            .collect(),
        total: response.brands.total,
        page: response.brands.page,
        per_page: response.brands.per_page,
    })
}

pub async fn load_detail(
    access_token: Option<String>,
    tenant_slug: Option<String>,
    brand_id: String,
) -> Result<BrandAdminRecord, GraphqlBrandAdminError> {
    let response: DetailResponse = request(
        DETAIL_QUERY,
        serde_json::json!({ "id": brand_id }),
        access_token,
        tenant_slug,
    )
    .await?;

    let item = response.brand.ok_or_else(|| "Brand not found".to_string())?;

    Ok(BrandAdminRecord {
        id: item.id,
        tenant_id: item.tenant_id,
        slug: item.slug,
        name: item.name,
        description: item.description,
        website_url: item.website_url,
        logo_url: item.logo_url,
        metadata: item.metadata,
        is_active: item.is_active,
        sort_order: item.sort_order,
        created_at: item.created_at,
        updated_at: item.updated_at,
        translations: Vec::new(),
    })
}

pub async fn execute_command(
    access_token: Option<String>,
    tenant_slug: Option<String>,
    _idempotency_key: String,
    command: BrandAdminCommand,
) -> Result<BrandAdminCommandResult, GraphqlBrandAdminError> {
    match command {
        BrandAdminCommand::Create { draft } => {
            let input = serde_json::json!({
                "slug": draft.slug,
                "name": draft.name,
                "description": draft.description,
                "websiteUrl": draft.website_url,
                "logoUrl": draft.logo_url,
                "isActive": draft.is_active,
                "sortOrder": draft.sort_order,
            });
            let response: CreateResponse = request(
                CREATE_MUTATION,
                serde_json::json!({ "input": input }),
                access_token,
                tenant_slug,
            )
            .await?;

            Ok(BrandAdminCommandResult {
                brand: Some(BrandAdminRecord {
                    id: response.brand.id,
                    tenant_id: response.brand.tenant_id,
                    slug: response.brand.slug,
                    name: response.brand.name,
                    description: response.brand.description,
                    website_url: response.brand.website_url,
                    logo_url: response.brand.logo_url,
                    metadata: response.brand.metadata,
                    is_active: response.brand.is_active,
                    sort_order: response.brand.sort_order,
                    created_at: response.brand.created_at,
                    updated_at: response.brand.updated_at,
                    translations: Vec::new(),
                }),
                success: true,
            })
        }
        BrandAdminCommand::Update { id, draft } => {
            let input = serde_json::json!({
                "slug": draft.slug,
                "name": draft.name,
                "description": draft.description,
                "websiteUrl": draft.website_url,
                "logoUrl": draft.logo_url,
                "isActive": draft.is_active,
                "sortOrder": draft.sort_order,
            });
            let response: UpdateResponse = request(
                UPDATE_MUTATION,
                serde_json::json!({ "id": id, "input": input }),
                access_token,
                tenant_slug,
            )
            .await?;

            Ok(BrandAdminCommandResult {
                brand: Some(BrandAdminRecord {
                    id: response.brand.id,
                    tenant_id: response.brand.tenant_id,
                    slug: response.brand.slug,
                    name: response.brand.name,
                    description: response.brand.description,
                    website_url: response.brand.website_url,
                    logo_url: response.brand.logo_url,
                    metadata: response.brand.metadata,
                    is_active: response.brand.is_active,
                    sort_order: response.brand.sort_order,
                    created_at: response.brand.created_at,
                    updated_at: response.brand.updated_at,
                    translations: Vec::new(),
                }),
                success: true,
            })
        }
        BrandAdminCommand::Delete { id } => {
            let response: DeleteResponse = request(
                DELETE_MUTATION,
                serde_json::json!({ "id": id }),
                access_token,
                tenant_slug,
            )
            .await?;

            Ok(BrandAdminCommandResult {
                brand: None,
                success: response.success,
            })
        }
        BrandAdminCommand::SetTranslation { .. } => Ok(BrandAdminCommandResult {
            brand: None,
            success: true,
        }),
    }
}
