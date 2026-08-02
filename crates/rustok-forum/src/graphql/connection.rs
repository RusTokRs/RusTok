use async_graphql::SimpleObject;
use rustok_api::graphql::PageInfo;

#[derive(SimpleObject, Debug, Clone)]
#[graphql(concrete(
    name = "ForumCategoryConnection",
    params(crate::graphql::GqlForumCategory)
))]
#[graphql(concrete(
    name = "ForumTopicConnection",
    params(crate::graphql::GqlForumTopicListItem)
))]
#[graphql(concrete(name = "ForumReplyConnection", params(crate::graphql::GqlForumReply)))]
pub struct ListConnection<T>
where
    T: async_graphql::OutputType,
{
    pub items: Vec<T>,
    pub page_info: PageInfo,
}

impl<T> ListConnection<T>
where
    T: async_graphql::OutputType,
{
    pub fn new(items: Vec<T>, total: i64, offset: i64, limit: i64) -> Self {
        Self {
            items,
            page_info: PageInfo::new(total, offset, limit),
        }
    }
}
