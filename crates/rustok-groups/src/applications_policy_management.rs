#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupApplicationPolicyLocalesRequest {
    pub group_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadGroupApplicationPolicyForManagementRequest {
    pub group_id: Uuid,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicyLocaleCatalog {
    pub group_id: Uuid,
    pub policy_id: Option<Uuid>,
    pub revision: Option<u64>,
    pub enabled: bool,
    pub locales: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicyManagementView {
    pub group_id: Uuid,
    pub policy_id: Option<Uuid>,
    pub revision: Option<u64>,
    pub enabled: bool,
    pub locale: String,
    pub translation_exists: bool,
    pub questions: Vec<GroupApplicationQuestion>,
    pub rules: Vec<GroupApplicationRule>,
}

#[async_trait]
pub trait GroupApplicationPolicyManagementReadPort: Send + Sync {
    async fn list_group_application_policy_locales(
        &self,
        context: PortContext,
        request: ListGroupApplicationPolicyLocalesRequest,
    ) -> Result<GroupApplicationPolicyLocaleCatalog, PortError>;

    async fn read_group_application_policy_for_management(
        &self,
        context: PortContext,
        request: ReadGroupApplicationPolicyForManagementRequest,
    ) -> Result<GroupApplicationPolicyManagementView, PortError>;
}

impl GroupApplicationService {
    async fn list_policy_locales_owned(
        &self,
        context: &PortContext,
        request: ListGroupApplicationPolicyLocalesRequest,
    ) -> GroupsResult<GroupApplicationPolicyLocaleCatalog> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let group_model = find_group(&self.db, tenant_id, request.group_id).await?;
        require_active_group(&group_model)?;
        authorize_policy_management(
            &self.db,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;

        let policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.group_id))
            .one(&self.db)
            .await?;
        let Some(policy) = policy else {
            return Ok(GroupApplicationPolicyLocaleCatalog {
                group_id: request.group_id,
                policy_id: None,
                revision: None,
                enabled: true,
                locales: Vec::new(),
            });
        };

        let locales = membership_policy_translation::Entity::find()
            .filter(membership_policy_translation::Column::TenantId.eq(tenant_id))
            .filter(membership_policy_translation::Column::PolicyId.eq(policy.id))
            .order_by_asc(membership_policy_translation::Column::Locale)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|translation| translation.locale)
            .collect();

        Ok(GroupApplicationPolicyLocaleCatalog {
            group_id: request.group_id,
            policy_id: Some(policy.id),
            revision: Some(policy.revision.max(1) as u64),
            enabled: policy.enabled,
            locales,
        })
    }

    async fn read_policy_for_management_owned(
        &self,
        context: &PortContext,
        mut request: ReadGroupApplicationPolicyForManagementRequest,
    ) -> GroupsResult<GroupApplicationPolicyManagementView> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.locale = normalize_locale_tag(&request.locale).ok_or_else(|| {
            GroupsError::Validation("invalid application policy management locale".to_string())
        })?;
        let group_model = find_group(&self.db, tenant_id, request.group_id).await?;
        require_active_group(&group_model)?;
        authorize_policy_management(
            &self.db,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;

        let policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.group_id))
            .one(&self.db)
            .await?;
        let Some(policy) = policy else {
            return Ok(GroupApplicationPolicyManagementView {
                group_id: request.group_id,
                policy_id: None,
                revision: None,
                enabled: true,
                locale: request.locale,
                translation_exists: false,
                questions: Vec::new(),
                rules: Vec::new(),
            });
        };

        let translation = membership_policy_translation::Entity::find()
            .filter(membership_policy_translation::Column::TenantId.eq(tenant_id))
            .filter(membership_policy_translation::Column::PolicyId.eq(policy.id))
            .filter(
                membership_policy_translation::Column::Locale.eq(request.locale.clone()),
            )
            .one(&self.db)
            .await?;

        let (translation_exists, questions, rules) = match translation {
            Some(translation) => (
                true,
                serde_json::from_value::<Vec<GroupApplicationQuestion>>(translation.questions)
                    .map_err(|error| {
                        GroupsError::Invariant(format!(
                            "invalid application policy management questions: {error}"
                        ))
                    })?,
                serde_json::from_value::<Vec<GroupApplicationRule>>(translation.rules).map_err(
                    |error| {
                        GroupsError::Invariant(format!(
                            "invalid application policy management rules: {error}"
                        ))
                    },
                )?,
            ),
            None => (false, Vec::new(), Vec::new()),
        };

        Ok(GroupApplicationPolicyManagementView {
            group_id: request.group_id,
            policy_id: Some(policy.id),
            revision: Some(policy.revision.max(1) as u64),
            enabled: policy.enabled,
            locale: request.locale,
            translation_exists,
            questions,
            rules,
        })
    }
}

#[async_trait]
impl GroupApplicationPolicyManagementReadPort for GroupApplicationService {
    async fn list_group_application_policy_locales(
        &self,
        context: PortContext,
        request: ListGroupApplicationPolicyLocalesRequest,
    ) -> Result<GroupApplicationPolicyLocaleCatalog, PortError> {
        self.list_policy_locales_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn read_group_application_policy_for_management(
        &self,
        context: PortContext,
        request: ReadGroupApplicationPolicyForManagementRequest,
    ) -> Result<GroupApplicationPolicyManagementView, PortError> {
        self.read_policy_for_management_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}
