use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::model::{
    ProductRelationItem, ProductRelationsAdminCommand, ProductRelationsAdminCommandResult,
    ProductRelationsAdminFilters,
};

pub type GraphqlProductRelationsAdminError = String;

const FETCH_RELATIONS_QUERY: &str = r#"
query ProductAdminProductRelations($productId: UUID!, $relationType: GqlRelationType) {
  productRelations(productId: $productId, relationType: $relationType) {
    id
    productId
    relatedProductId
    relationType
    position
    metadata
    createdAt
    updatedAt
  }
}
"#;

const ADD_RELATION_MUTATION: &str = r#"
mutation ProductAdminAddRelation($input: AddProductRelationInput!) {
  addProductRelation(input: $input) {
    id
    productId
    relatedProductId
    relationType
    position
    metadata
    createdAt
    updatedAt
  }
}
"#;

const REMOVE_RELATION_MUTATION: &str = r#"
mutation ProductAdminRemoveRelation($id: UUID!) {
  removeProductRelation(id: $id)
}
"#;

const REORDER_RELATIONS_MUTATION: &str = r#"
mutation ProductAdminReorderRelations($productId: UUID!, $relationType: GqlRelationType!, $orderedRelationIds: [UUID!]!) {
  reorderProductRelations(productId: $productId, relationType: $relationType, orderedRelationIds: $orderedRelationIds) {
    id
    productId
    relatedProductId
    relationType
    position
    metadata
    createdAt
    updatedAt
  }
}
"#;

#[derive(Debug, Deserialize)]
struct FetchResponse {
    #[serde(rename = "productRelations")]
    product_relations: Vec<ProductRelationItem>,
}

#[derive(Debug, Deserialize)]
struct AddResponse {
    #[serde(rename = "addProductRelation")]
    add_product_relation: ProductRelationItem,
}

#[derive(Debug, Deserialize)]
struct RemoveResponse {
    #[serde(rename = "removeProductRelation")]
    remove_product_relation: bool,
}

#[derive(Debug, Deserialize)]
struct ReorderResponse {
    #[serde(rename = "reorderProductRelations")]
    reorder_product_relations: Vec<ProductRelationItem>,
}

fn relation_type_to_gql(rtype: &str) -> &'static str {
    match rtype {
        "cross_sell" => "CROSS_SELL",
        "up_sell" => "UP_SELL",
        "related" => "RELATED",
        "accessory" => "ACCESSORY",
        "alternative" => "ALTERNATIVE",
        _ => "RELATED",
    }
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, GraphqlProductRelationsAdminError>
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


pub async fn load_relations(
    token: Option<String>,
    tenant_slug: Option<String>,
    filters: ProductRelationsAdminFilters,
) -> Result<Vec<ProductRelationItem>, GraphqlProductRelationsAdminError> {
    #[derive(Serialize)]
    struct Vars {
        #[serde(rename = "productId")]
        product_id: String,
        #[serde(rename = "relationType", skip_serializing_if = "Option::is_none")]
        relation_type: Option<String>,
    }

    let product_id = filters
        .product_id
        .ok_or_else(|| "product_id is required".to_string())?;

    let vars = Vars {
        product_id,
        relation_type: filters.relation_type.as_deref().map(relation_type_to_gql).map(str::to_string),
    };

    let data: FetchResponse = request(FETCH_RELATIONS_QUERY, vars, token, tenant_slug).await?;
    Ok(data.product_relations)
}

pub async fn execute_command(
    token: Option<String>,
    tenant_slug: Option<String>,
    _idempotency_key: String,
    command: ProductRelationsAdminCommand,
) -> Result<ProductRelationsAdminCommandResult, GraphqlProductRelationsAdminError> {
    match command {
        ProductRelationsAdminCommand::Add { draft } => {
            #[derive(Serialize)]
            struct AddInput {
                #[serde(rename = "productId")]
                product_id: String,
                #[serde(rename = "relatedProductId")]
                related_product_id: String,
                #[serde(rename = "relationType")]
                relation_type: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                position: Option<i32>,
            }
            #[derive(Serialize)]
            struct Vars {
                input: AddInput,
            }

            let vars = Vars {
                input: AddInput {
                    product_id: draft.product_id,
                    related_product_id: draft.related_product_id,
                    relation_type: relation_type_to_gql(&draft.relation_type).to_string(),
                    position: draft.position,
                },
            };

            let data: AddResponse = request(ADD_RELATION_MUTATION, vars, token, tenant_slug).await?;
            Ok(ProductRelationsAdminCommandResult {
                item: Some(data.add_product_relation),
                items: vec![],
                success: true,
            })
        }
        ProductRelationsAdminCommand::Remove { id } => {
            #[derive(Serialize)]
            struct Vars {
                id: String,
            }
            let data: RemoveResponse = request(REMOVE_RELATION_MUTATION, Vars { id }, token, tenant_slug).await?;
            Ok(ProductRelationsAdminCommandResult {
                item: None,
                items: vec![],
                success: data.remove_product_relation,
            })
        }
        ProductRelationsAdminCommand::Reorder {
            product_id,
            relation_type,
            ordered_ids,
        } => {
            #[derive(Serialize)]
            struct Vars {
                #[serde(rename = "productId")]
                product_id: String,
                #[serde(rename = "relationType")]
                relation_type: String,
                #[serde(rename = "orderedRelationIds")]
                ordered_relation_ids: Vec<String>,
            }

            let vars = Vars {
                product_id,
                relation_type: relation_type_to_gql(&relation_type).to_string(),
                ordered_relation_ids: ordered_ids,
            };

            let data: ReorderResponse = request(REORDER_RELATIONS_MUTATION, vars, token, tenant_slug).await?;
            Ok(ProductRelationsAdminCommandResult {
                item: None,
                items: data.reorder_product_relations,
                success: true,
            })
        }
    }
}
