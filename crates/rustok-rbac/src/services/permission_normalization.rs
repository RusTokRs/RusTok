use rustok_api::Permission;

pub(crate) fn normalize_permissions(mut permissions: Vec<Permission>) -> Vec<Permission> {
    permissions.sort_unstable_by_key(|permission| permission.to_string());
    permissions.dedup();
    permissions
}

#[cfg(test)]
mod tests {
    use super::normalize_permissions;
    use rustok_api::Permission;

    #[test]
    fn normalize_permissions_deduplicates_and_sorts() {
        let normalized = normalize_permissions(vec![
            Permission::USERS_UPDATE,
            Permission::USERS_READ,
            Permission::USERS_UPDATE,
        ]);

        assert_eq!(
            normalized,
            vec![Permission::USERS_READ, Permission::USERS_UPDATE]
        );
    }
}
