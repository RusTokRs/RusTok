use async_graphql::SimpleObject;
use rustok_api::graphql::PageInfo;

#[derive(SimpleObject, Debug, Clone)]
#[graphql(concrete(
    name = "ForumCategoryConnection",
    params(crate::graphql::GqlForumCategory)
))]
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

/// One keyset page of forum topics. Topics are not counted; `next_cursor` is present
/// only when another page exists.
#[derive(SimpleObject, Debug, Clone)]
pub struct ForumTopicPage {
    pub items: Vec<crate::graphql::GqlForumTopicListItem>,
    pub next_cursor: Option<String>,
}

/// One keyset page of forum replies. Replies are not counted; `next_cursor` is
/// present only when another page exists.
#[derive(SimpleObject, Debug, Clone)]
pub struct ForumReplyPage {
    pub items: Vec<crate::graphql::GqlForumReply>,
    pub next_cursor: Option<String>,
}
