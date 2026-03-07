use async_graphql::SimpleObject;

use crate::graphql::common::PageInfo;

#[derive(SimpleObject, Debug, Clone)]
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
