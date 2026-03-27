pub const USERS_QUERY: &str = "query Users($pagination: PaginationInput, $filter: UsersFilter, $search: String) { users(pagination: $pagination, filter: $filter, search: $search) { edges { cursor node { id email name role status createdAt tenantName } } pageInfo { totalCount hasNextPage endCursor } } }";

pub const USERS_QUERY_HASH: &str =
    "ff1e132e28d2e1c804d8d5ade5966307e17685b9f4b39262d70ecaa4d49abb66";

pub const USER_DETAILS_QUERY: &str =
    "query User($id: UUID!) { user(id: $id) { id email name role status createdAt tenantName } }";

pub const USER_DETAILS_QUERY_HASH: &str =
    "85f7f7ba212ab47e951fcf7dbb30bb918e66b88710574a576b0088877653f3b7";

pub const DASHBOARD_STATS_QUERY: &str =
    "query DashboardStats { dashboardStats { totalUsers totalPosts totalOrders totalRevenue usersChange postsChange ordersChange revenueChange } }";

pub const RECENT_ACTIVITY_QUERY: &str = "query RecentActivity($limit: Int!) { recentActivity(limit: $limit) { id type description timestamp user { id name } } }";

pub const RECENT_ACTIVITY_QUERY_HASH: &str =
    "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t0u1v2w3x4y5z6a7b8c9d0e1f2";

pub const ADMIN_GLOBAL_SEARCH_QUERY: &str = "query AdminGlobalSearch($input: SearchPreviewInput!) { adminGlobalSearch(input: $input) { queryLogId total tookMs engine items { id entityType sourceModule title snippet score locale url payload } } }";

pub const CREATE_USER_MUTATION: &str = r#"
mutation CreateUser($input: CreateUserInput!) {
    createUser(input: $input) {
        id email name role status createdAt tenantName
    }
}
"#;

pub const ROLES_QUERY: &str = "query Roles { roles { slug displayName permissions } }";

pub const PLATFORM_SETTINGS_QUERY: &str =
    "query PlatformSettings($category: String!) { platformSettings(category: $category) { category settings } }";

pub const UPDATE_PLATFORM_SETTINGS_MUTATION: &str = r#"
mutation UpdatePlatformSettings($input: UpdatePlatformSettingsInput!) {
    updatePlatformSettings(input: $input) {
        success category
    }
}
"#;

pub const CACHE_HEALTH_QUERY: &str =
    "query CacheHealth { cacheHealth { redisConfigured redisHealthy redisError backend } }";

pub const EVENTS_STATUS_QUERY: &str =
    "query EventsStatus { eventsStatus { configuredTransport iggyMode relayIntervalMs dlqEnabled maxAttempts pendingEvents dlqEvents availableTransports } }";
