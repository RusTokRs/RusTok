use async_graphql::SimpleObject;

use crate::graphql::common::PageInfo;

#[derive(SimpleObject, Debug, Clone)]
#[cfg_attr(
    feature = "mod-forum",
    graphql(concrete(
        name = "ForumCategoryConnection",
        params(crate::graphql::forum::GqlForumCategory)
    ))
)]
#[cfg_attr(
    feature = "mod-forum",
    graphql(concrete(
        name = "ForumTopicConnection",
        params(crate::graphql::forum::GqlForumTopic)
    ))
)]
#[cfg_attr(
    feature = "mod-forum",
    graphql(concrete(
        name = "ForumReplyConnection",
        params(crate::graphql::forum::GqlForumReply)
    ))
)]
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
