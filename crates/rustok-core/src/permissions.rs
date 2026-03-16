use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Ресурсы системы
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    Users,
    Tenants,
    Modules,
    Settings,
    Products,
    Categories,
    Orders,
    Customers,
    Inventory,
    Discounts,
    Posts,
    Pages,
    Nodes,
    Media,
    Comments,
    Tags,
    Analytics,
    Logs,
    Webhooks,
    // Blog domain resources
    BlogPosts,
    // Forum domain resources
    ForumCategories,
    ForumTopics,
    ForumReplies,
    // Scripting (alloy)
    Scripts,
    // Workflow automation
    Workflows,
    WorkflowExecutions,
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Users => "users",
            Self::Tenants => "tenants",
            Self::Modules => "modules",
            Self::Settings => "settings",
            Self::Products => "products",
            Self::Categories => "categories",
            Self::Orders => "orders",
            Self::Customers => "customers",
            Self::Inventory => "inventory",
            Self::Discounts => "discounts",
            Self::Posts => "posts",
            Self::Pages => "pages",
            Self::Nodes => "nodes",
            Self::Media => "media",
            Self::Comments => "comments",
            Self::Tags => "tags",
            Self::Analytics => "analytics",
            Self::Logs => "logs",
            Self::Webhooks => "webhooks",
            Self::BlogPosts => "blog_posts",
            Self::ForumCategories => "forum_categories",
            Self::ForumTopics => "forum_topics",
            Self::ForumReplies => "forum_replies",
            Self::Scripts => "scripts",
            Self::Workflows => "workflows",
            Self::WorkflowExecutions => "workflow_executions",
        };
        write!(f, "{value}")
    }
}

impl FromStr for Resource {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "users" => Ok(Self::Users),
            "tenants" => Ok(Self::Tenants),
            "modules" => Ok(Self::Modules),
            "settings" => Ok(Self::Settings),
            "products" => Ok(Self::Products),
            "categories" => Ok(Self::Categories),
            "orders" => Ok(Self::Orders),
            "customers" => Ok(Self::Customers),
            "inventory" => Ok(Self::Inventory),
            "discounts" => Ok(Self::Discounts),
            "posts" => Ok(Self::Posts),
            "pages" => Ok(Self::Pages),
            "nodes" => Ok(Self::Nodes),
            "media" => Ok(Self::Media),
            "comments" => Ok(Self::Comments),
            "tags" => Ok(Self::Tags),
            "analytics" => Ok(Self::Analytics),
            "logs" => Ok(Self::Logs),
            "webhooks" => Ok(Self::Webhooks),
            "blog_posts" => Ok(Self::BlogPosts),
            "forum_categories" => Ok(Self::ForumCategories),
            "forum_topics" => Ok(Self::ForumTopics),
            "forum_replies" => Ok(Self::ForumReplies),
            "scripts" => Ok(Self::Scripts),
            "workflows" => Ok(Self::Workflows),
            "workflow_executions" => Ok(Self::WorkflowExecutions),
            _ => Err(format!("Unknown resource: {value}")),
        }
    }
}

/// Действия над ресурсами
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Create,
    Read,
    Update,
    Delete,
    List,
    Export,
    Import,
    Manage,
    Publish,
    Moderate,
    Execute,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Create => "create",
            Self::Read => "read",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::List => "list",
            Self::Export => "export",
            Self::Import => "import",
            Self::Manage => "manage",
            Self::Publish => "publish",
            Self::Moderate => "moderate",
            Self::Execute => "execute",
        };
        write!(f, "{value}")
    }
}

impl FromStr for Action {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "create" => Ok(Self::Create),
            "read" => Ok(Self::Read),
            "update" => Ok(Self::Update),
            "delete" => Ok(Self::Delete),
            "list" => Ok(Self::List),
            "export" => Ok(Self::Export),
            "import" => Ok(Self::Import),
            "manage" | "*" => Ok(Self::Manage),
            "publish" => Ok(Self::Publish),
            "moderate" => Ok(Self::Moderate),
            "execute" => Ok(Self::Execute),
            _ => Err(format!("Unknown action: {value}")),
        }
    }
}

/// Permission = Resource + Action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    pub resource: Resource,
    pub action: Action,
}

impl Permission {
    pub const fn new(resource: Resource, action: Action) -> Self {
        Self { resource, action }
    }
}

impl FromStr for Permission {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split(':');
        let resource = Resource::from_str(parts.next().ok_or("Missing resource")?)?;
        let action = Action::from_str(parts.next().ok_or("Missing action")?)?;
        if parts.next().is_some() {
            return Err("Too many parts in permission string".to_string());
        }
        Ok(Self { resource, action })
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.resource, self.action)
    }
}

impl Permission {
    pub const USERS_CREATE: Self = Self::new(Resource::Users, Action::Create);
    pub const USERS_READ: Self = Self::new(Resource::Users, Action::Read);
    pub const USERS_UPDATE: Self = Self::new(Resource::Users, Action::Update);
    pub const USERS_DELETE: Self = Self::new(Resource::Users, Action::Delete);
    pub const USERS_LIST: Self = Self::new(Resource::Users, Action::List);
    pub const USERS_MANAGE: Self = Self::new(Resource::Users, Action::Manage);

    pub const PRODUCTS_CREATE: Self = Self::new(Resource::Products, Action::Create);
    pub const PRODUCTS_READ: Self = Self::new(Resource::Products, Action::Read);
    pub const PRODUCTS_UPDATE: Self = Self::new(Resource::Products, Action::Update);
    pub const PRODUCTS_DELETE: Self = Self::new(Resource::Products, Action::Delete);
    pub const PRODUCTS_LIST: Self = Self::new(Resource::Products, Action::List);
    pub const PRODUCTS_MANAGE: Self = Self::new(Resource::Products, Action::Manage);

    pub const ORDERS_CREATE: Self = Self::new(Resource::Orders, Action::Create);
    pub const ORDERS_READ: Self = Self::new(Resource::Orders, Action::Read);
    pub const ORDERS_UPDATE: Self = Self::new(Resource::Orders, Action::Update);
    pub const ORDERS_DELETE: Self = Self::new(Resource::Orders, Action::Delete);
    pub const ORDERS_LIST: Self = Self::new(Resource::Orders, Action::List);
    pub const ORDERS_MANAGE: Self = Self::new(Resource::Orders, Action::Manage);

    pub const POSTS_CREATE: Self = Self::new(Resource::Posts, Action::Create);
    pub const POSTS_READ: Self = Self::new(Resource::Posts, Action::Read);
    pub const POSTS_UPDATE: Self = Self::new(Resource::Posts, Action::Update);
    pub const POSTS_DELETE: Self = Self::new(Resource::Posts, Action::Delete);
    pub const POSTS_LIST: Self = Self::new(Resource::Posts, Action::List);
    pub const POSTS_MANAGE: Self = Self::new(Resource::Posts, Action::Manage);

    pub const NODES_CREATE: Self = Self::new(Resource::Nodes, Action::Create);
    pub const NODES_READ: Self = Self::new(Resource::Nodes, Action::Read);
    pub const NODES_UPDATE: Self = Self::new(Resource::Nodes, Action::Update);
    pub const NODES_DELETE: Self = Self::new(Resource::Nodes, Action::Delete);
    pub const NODES_LIST: Self = Self::new(Resource::Nodes, Action::List);
    pub const NODES_MANAGE: Self = Self::new(Resource::Nodes, Action::Manage);

    pub const PAGES_CREATE: Self = Self::new(Resource::Pages, Action::Create);
    pub const PAGES_READ: Self = Self::new(Resource::Pages, Action::Read);
    pub const PAGES_UPDATE: Self = Self::new(Resource::Pages, Action::Update);
    pub const PAGES_DELETE: Self = Self::new(Resource::Pages, Action::Delete);
    pub const PAGES_LIST: Self = Self::new(Resource::Pages, Action::List);
    pub const PAGES_MANAGE: Self = Self::new(Resource::Pages, Action::Manage);

    pub const SETTINGS_READ: Self = Self::new(Resource::Settings, Action::Read);
    pub const SETTINGS_UPDATE: Self = Self::new(Resource::Settings, Action::Update);
    pub const SETTINGS_MANAGE: Self = Self::new(Resource::Settings, Action::Manage);

    pub const ANALYTICS_READ: Self = Self::new(Resource::Analytics, Action::Read);
    pub const ANALYTICS_EXPORT: Self = Self::new(Resource::Analytics, Action::Export);

    pub const BLOG_POSTS_CREATE: Self = Self::new(Resource::BlogPosts, Action::Create);
    pub const BLOG_POSTS_READ: Self = Self::new(Resource::BlogPosts, Action::Read);
    pub const BLOG_POSTS_UPDATE: Self = Self::new(Resource::BlogPosts, Action::Update);
    pub const BLOG_POSTS_DELETE: Self = Self::new(Resource::BlogPosts, Action::Delete);
    pub const BLOG_POSTS_LIST: Self = Self::new(Resource::BlogPosts, Action::List);
    pub const BLOG_POSTS_PUBLISH: Self = Self::new(Resource::BlogPosts, Action::Publish);
    pub const BLOG_POSTS_MANAGE: Self = Self::new(Resource::BlogPosts, Action::Manage);

    pub const FORUM_CATEGORIES_CREATE: Self = Self::new(Resource::ForumCategories, Action::Create);
    pub const FORUM_CATEGORIES_READ: Self = Self::new(Resource::ForumCategories, Action::Read);
    pub const FORUM_CATEGORIES_UPDATE: Self = Self::new(Resource::ForumCategories, Action::Update);
    pub const FORUM_CATEGORIES_DELETE: Self = Self::new(Resource::ForumCategories, Action::Delete);
    pub const FORUM_CATEGORIES_LIST: Self = Self::new(Resource::ForumCategories, Action::List);
    pub const FORUM_CATEGORIES_MANAGE: Self = Self::new(Resource::ForumCategories, Action::Manage);

    pub const FORUM_TOPICS_CREATE: Self = Self::new(Resource::ForumTopics, Action::Create);
    pub const FORUM_TOPICS_READ: Self = Self::new(Resource::ForumTopics, Action::Read);
    pub const FORUM_TOPICS_UPDATE: Self = Self::new(Resource::ForumTopics, Action::Update);
    pub const FORUM_TOPICS_DELETE: Self = Self::new(Resource::ForumTopics, Action::Delete);
    pub const FORUM_TOPICS_LIST: Self = Self::new(Resource::ForumTopics, Action::List);
    pub const FORUM_TOPICS_MODERATE: Self = Self::new(Resource::ForumTopics, Action::Moderate);
    pub const FORUM_TOPICS_MANAGE: Self = Self::new(Resource::ForumTopics, Action::Manage);

    pub const FORUM_REPLIES_CREATE: Self = Self::new(Resource::ForumReplies, Action::Create);
    pub const FORUM_REPLIES_READ: Self = Self::new(Resource::ForumReplies, Action::Read);
    pub const FORUM_REPLIES_UPDATE: Self = Self::new(Resource::ForumReplies, Action::Update);
    pub const FORUM_REPLIES_DELETE: Self = Self::new(Resource::ForumReplies, Action::Delete);
    pub const FORUM_REPLIES_LIST: Self = Self::new(Resource::ForumReplies, Action::List);
    pub const FORUM_REPLIES_MODERATE: Self = Self::new(Resource::ForumReplies, Action::Moderate);
    pub const FORUM_REPLIES_MANAGE: Self = Self::new(Resource::ForumReplies, Action::Manage);

    pub const INVENTORY_CREATE: Self = Self::new(Resource::Inventory, Action::Create);
    pub const INVENTORY_READ: Self = Self::new(Resource::Inventory, Action::Read);
    pub const INVENTORY_UPDATE: Self = Self::new(Resource::Inventory, Action::Update);
    pub const INVENTORY_DELETE: Self = Self::new(Resource::Inventory, Action::Delete);
    pub const INVENTORY_LIST: Self = Self::new(Resource::Inventory, Action::List);
    pub const INVENTORY_MANAGE: Self = Self::new(Resource::Inventory, Action::Manage);

    pub const SCRIPTS_CREATE: Self = Self::new(Resource::Scripts, Action::Create);
    pub const SCRIPTS_READ: Self = Self::new(Resource::Scripts, Action::Read);
    pub const SCRIPTS_UPDATE: Self = Self::new(Resource::Scripts, Action::Update);
    pub const SCRIPTS_DELETE: Self = Self::new(Resource::Scripts, Action::Delete);
    pub const SCRIPTS_LIST: Self = Self::new(Resource::Scripts, Action::List);
    pub const SCRIPTS_MANAGE: Self = Self::new(Resource::Scripts, Action::Manage);

    pub const WORKFLOWS_CREATE: Self = Self::new(Resource::Workflows, Action::Create);
    pub const WORKFLOWS_READ: Self = Self::new(Resource::Workflows, Action::Read);
    pub const WORKFLOWS_UPDATE: Self = Self::new(Resource::Workflows, Action::Update);
    pub const WORKFLOWS_DELETE: Self = Self::new(Resource::Workflows, Action::Delete);
    pub const WORKFLOWS_LIST: Self = Self::new(Resource::Workflows, Action::List);
    pub const WORKFLOWS_EXECUTE: Self = Self::new(Resource::Workflows, Action::Execute);
    pub const WORKFLOWS_MANAGE: Self = Self::new(Resource::Workflows, Action::Manage);

    pub const WORKFLOW_EXECUTIONS_READ: Self =
        Self::new(Resource::WorkflowExecutions, Action::Read);
    pub const WORKFLOW_EXECUTIONS_LIST: Self =
        Self::new(Resource::WorkflowExecutions, Action::List);
}
