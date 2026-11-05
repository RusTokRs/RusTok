use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DashboardStatsResponse {
    #[serde(rename = "dashboardStats")]
    pub dashboard_stats: Option<DashboardStats>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DashboardStats {
    #[serde(rename = "totalUsers")]
    pub total_users: i64,
    #[serde(rename = "usersChange")]
    pub users_change: f64,
    #[serde(default, rename = "totalPosts")]
    pub total_posts: Option<i64>,
    #[serde(default, rename = "totalOrders")]
    pub total_orders: Option<i64>,
    #[serde(default, rename = "totalRevenue")]
    pub total_revenue: Option<i64>,
    #[serde(default, rename = "postsChange")]
    pub posts_change: Option<f64>,
    #[serde(default, rename = "ordersChange")]
    pub orders_change: Option<f64>,
    #[serde(default, rename = "revenueChange")]
    pub revenue_change: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RecentActivityResponse {
    #[serde(rename = "recentActivity")]
    pub recent_activity: Vec<ActivityItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityItem {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub description: String,
    pub timestamp: String,
    pub user: Option<ActivityUser>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivityUser {
    pub id: String,
    pub name: Option<String>,
}
