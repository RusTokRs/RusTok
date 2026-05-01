use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, JoinType, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
    PLATFORM_FALLBACK_LOCALE,
};
use rustok_core::{Action, PermissionScope, Resource, SecurityContext};

use crate::dto::{
    CreateTaxonomyTermInput, ListTaxonomyTermsFilter, TaxonomyScopeType, TaxonomyTermKind,
    TaxonomyTermListItem, TaxonomyTermResponse, TaxonomyTermStatus, UpdateTaxonomyTermInput,
};
use crate::entities::{taxonomy_term, taxonomy_term_alias, taxonomy_term_translation};
use crate::error::{TaxonomyError, TaxonomyResult};

pub struct TaxonomyService {
    db: DatabaseConnection,
}

impl TaxonomyService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create_term(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTaxonomyTermInput,
    ) -> TaxonomyResult<Uuid> {
        enforce_scope(&security, Resource::Taxonomy, Action::Create)?;

        let locale = normalize_locale(&input.locale)?;
        let scope_value = normalize_scope_value(input.scope_type, input.scope_value.as_deref())?;
        let canonical_key = normalize_term_slug(
            input
                .canonical_key
                .as_deref()
                .or(input.slug.as_deref())
                .unwrap_or(&input.name),
        );
        if canonical_key.is_empty() {
            return Err(TaxonomyError::validation(
                "Canonical key cannot be empty after normalization",
            ));
        }

        validate_term_name(&input.name)?;
        validate_optional_description(input.description.as_deref())?;

        let translation_slug = normalize_term_slug(input.slug.as_deref().unwrap_or(&input.name));
        if translation_slug.is_empty() {
            return Err(TaxonomyError::validation(
                "Localized slug cannot be empty after normalization",
            ));
        }

        let aliases = normalize_aliases(&input.aliases);
        let txn = self.db.begin().await?;

        self.ensure_canonical_key_available_in_tx(
            &txn,
            tenant_id,
            input.kind,
            input.scope_type,
            &scope_value,
            &canonical_key,
            None,
        )
        .await?;
        self.ensure_translation_slug_available_in_tx(
            &txn,
            tenant_id,
            input.kind,
            input.scope_type,
            &scope_value,
            &locale,
            &translation_slug,
            None,
        )
        .await?;
        self.ensure_aliases_available_in_tx(
            &txn,
            tenant_id,
            input.kind,
            input.scope_type,
            &scope_value,
            &locale,
            &aliases,
            None,
        )
        .await?;

        let now = Utc::now();
        let term_id = Uuid::new_v4();
        taxonomy_term::ActiveModel {
            id: Set(term_id),
            tenant_id: Set(tenant_id),
            kind: Set(input.kind),
            scope_type: Set(input.scope_type),
            scope_value: Set(scope_value.clone()),
            canonical_key: Set(canonical_key),
            status: Set(TaxonomyTermStatus::Active),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        taxonomy_term_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            term_id: Set(term_id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            name: Set(input.name),
            slug: Set(translation_slug),
            description: Set(input.description),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        self.replace_aliases_in_tx(&txn, tenant_id, term_id, &locale, &aliases)
            .await?;

        txn.commit().await?;
        Ok(term_id)
    }

    #[instrument(skip(self, security))]
    pub async fn get_term(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        term_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<TaxonomyTermResponse> {
        enforce_scope(&security, Resource::Taxonomy, Action::Read)?;

        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let term = self.find_term(tenant_id, term_id).await?;
        let translations = self.load_translations(term_id).await?;
        let aliases = self.load_aliases(term_id).await?;

        Ok(build_term_response(
            term,
            translations,
            aliases,
            &locale,
            fallback_locale.as_deref(),
        ))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_term(
        &self,
        tenant_id: Uuid,
        term_id: Uuid,
        security: SecurityContext,
        input: UpdateTaxonomyTermInput,
    ) -> TaxonomyResult<TaxonomyTermResponse> {
        enforce_scope(&security, Resource::Taxonomy, Action::Update)?;

        let locale = normalize_locale(&input.locale)?;
        let term = self.find_term(tenant_id, term_id).await?;
        let txn = self.db.begin().await?;
        let now = Utc::now();

        if let Some(status) = input.status {
            let mut active: taxonomy_term::ActiveModel = term.clone().into();
            active.status = Set(status);
            active.updated_at = Set(now.into());
            active.update(&txn).await?;
        }

        let existing_translation = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
            .filter(taxonomy_term_translation::Column::Locale.eq(&locale))
            .one(&txn)
            .await?;

        match existing_translation {
            Some(existing_translation) => {
                let mut active: taxonomy_term_translation::ActiveModel =
                    existing_translation.into();
                if let Some(name) = &input.name {
                    validate_term_name(name)?;
                    active.name = Set(name.clone());
                    if input.slug.is_none() {
                        let generated_slug = normalize_term_slug(name);
                        self.ensure_translation_slug_available_in_tx(
                            &txn,
                            tenant_id,
                            term.kind,
                            term.scope_type,
                            &term.scope_value,
                            &locale,
                            &generated_slug,
                            Some(term_id),
                        )
                        .await?;
                        active.slug = Set(generated_slug);
                    }
                }
                if let Some(slug) = input.slug.as_deref() {
                    let slug = normalize_term_slug(slug);
                    if slug.is_empty() {
                        return Err(TaxonomyError::validation(
                            "Localized slug cannot be empty after normalization",
                        ));
                    }
                    self.ensure_translation_slug_available_in_tx(
                        &txn,
                        tenant_id,
                        term.kind,
                        term.scope_type,
                        &term.scope_value,
                        &locale,
                        &slug,
                        Some(term_id),
                    )
                    .await?;
                    active.slug = Set(slug);
                }
                match input.description.clone() {
                    Some(description) => {
                        validate_optional_description(Some(&description))?;
                        active.description = Set(Some(description));
                    }
                    None if input.description.is_some() => {
                        active.description = Set(None);
                    }
                    None => {}
                }
                active.updated_at = Set(now.into());
                active.update(&txn).await?;
            }
            None => {
                let name = input.name.clone().ok_or_else(|| {
                    TaxonomyError::validation("Name is required when adding a new locale")
                })?;
                validate_term_name(&name)?;
                validate_optional_description(input.description.as_deref())?;
                let slug = normalize_term_slug(input.slug.as_deref().unwrap_or(&name));
                if slug.is_empty() {
                    return Err(TaxonomyError::validation(
                        "Localized slug cannot be empty after normalization",
                    ));
                }
                self.ensure_translation_slug_available_in_tx(
                    &txn,
                    tenant_id,
                    term.kind,
                    term.scope_type,
                    &term.scope_value,
                    &locale,
                    &slug,
                    Some(term_id),
                )
                .await?;

                taxonomy_term_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    term_id: Set(term_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(name),
                    slug: Set(slug),
                    description: Set(input.description.clone()),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await?;
            }
        }

        if let Some(aliases) = input.aliases.as_ref() {
            let aliases = normalize_aliases(aliases);
            self.ensure_aliases_available_in_tx(
                &txn,
                tenant_id,
                term.kind,
                term.scope_type,
                &term.scope_value,
                &locale,
                &aliases,
                Some(term_id),
            )
            .await?;
            self.replace_aliases_in_tx(&txn, tenant_id, term_id, &locale, &aliases)
                .await?;
        }

        txn.commit().await?;
        self.get_term(
            tenant_id,
            security,
            term_id,
            &locale,
            Some(PLATFORM_FALLBACK_LOCALE),
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn delete_term(
        &self,
        tenant_id: Uuid,
        term_id: Uuid,
        security: SecurityContext,
    ) -> TaxonomyResult<()> {
        enforce_scope(&security, Resource::Taxonomy, Action::Delete)?;
        let term = self.find_term(tenant_id, term_id).await?;
        term.delete(&self.db).await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list_terms(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTaxonomyTermsFilter,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<(Vec<TaxonomyTermListItem>, u64)> {
        enforce_scope(&security, Resource::Taxonomy, Action::List)?;

        let locale = filter.locale.as_deref().unwrap_or(PLATFORM_FALLBACK_LOCALE);
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let mut select =
            taxonomy_term::Entity::find().filter(taxonomy_term::Column::TenantId.eq(tenant_id));
        if let Some(kind) = filter.kind {
            select = select.filter(taxonomy_term::Column::Kind.eq(kind));
        }
        if let Some(scope_type) = filter.scope_type {
            select = select.filter(taxonomy_term::Column::ScopeType.eq(scope_type));
        }
        if let Some(scope_value) = filter.scope_value.as_deref() {
            select = select.filter(
                taxonomy_term::Column::ScopeValue.eq(normalize_optional_scope_label(scope_value)),
            );
        }
        if let Some(status) = filter.status {
            select = select.filter(taxonomy_term::Column::Status.eq(status));
        }

        let paginator = select
            .order_by_asc(taxonomy_term::Column::Kind)
            .order_by_asc(taxonomy_term::Column::ScopeType)
            .order_by_asc(taxonomy_term::Column::ScopeValue)
            .order_by_asc(taxonomy_term::Column::CanonicalKey)
            .paginate(&self.db, filter.per_page());
        let total = paginator.num_items().await?;
        let terms = paginator.fetch_page(filter.page() - 1).await?;

        if terms.is_empty() {
            return Ok((Vec::new(), total));
        }

        let term_ids: Vec<Uuid> = terms.iter().map(|term| term.id).collect();
        let translations_by_term = self.load_translations_map(&term_ids).await?;

        let items = terms
            .into_iter()
            .map(|term| {
                let translations = translations_by_term
                    .get(&term.id)
                    .cloned()
                    .unwrap_or_default();
                let resolved = resolve_by_locale_with_fallback(
                    &translations,
                    &locale,
                    fallback_locale.as_deref(),
                    |translation| translation.locale.as_str(),
                );

                TaxonomyTermListItem {
                    id: term.id,
                    kind: term.kind,
                    scope_type: term.scope_type,
                    scope_value: decode_scope_value(term.scope_type, &term.scope_value),
                    canonical_key: term.canonical_key,
                    status: term.status,
                    requested_locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    available_locales: available_locales_from(&translations, |translation| {
                        translation.locale.as_str()
                    }),
                    name: resolved
                        .item
                        .map(|translation| translation.name.clone())
                        .unwrap_or_default(),
                    slug: resolved
                        .item
                        .map(|translation| translation.slug.clone())
                        .unwrap_or_default(),
                    description: resolved
                        .item
                        .and_then(|translation| translation.description.clone()),
                    created_at: term.created_at.into(),
                }
            })
            .collect();

        Ok((items, total))
    }

    #[instrument(skip(self, txn, labels))]
    pub async fn ensure_terms_for_module_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        module_slug: &str,
        locale: &str,
        labels: &[String],
    ) -> TaxonomyResult<Vec<Uuid>> {
        let locale = normalize_locale(locale)?;
        let module_scope = normalize_scope_value(TaxonomyScopeType::Module, Some(module_slug))?;
        let mut term_ids = Vec::new();
        let mut seen = HashSet::new();

        for label in labels
            .iter()
            .map(|label| label.trim())
            .filter(|label| !label.is_empty())
        {
            validate_term_name(label)?;
            let normalized_slug = normalize_term_slug(label);
            if normalized_slug.is_empty() {
                continue;
            }

            let term_id = if let Some(term_id) = self
                .find_term_id_for_module_in_tx(
                    txn,
                    tenant_id,
                    kind,
                    &module_scope,
                    &locale,
                    &normalized_slug,
                )
                .await?
            {
                term_id
            } else {
                self.create_module_term_in_tx(
                    txn,
                    tenant_id,
                    kind,
                    &module_scope,
                    &locale,
                    label,
                    &normalized_slug,
                )
                .await?
            };

            if seen.insert(term_id) {
                term_ids.push(term_id);
            }
        }

        Ok(term_ids)
    }

    #[instrument(skip(self, term_ids))]
    pub async fn resolve_term_names(
        &self,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<HashMap<Uuid, String>> {
        if term_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let terms = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Id.is_in(term_ids.to_vec()))
            .all(&self.db)
            .await?;

        let translations_by_term = self.load_translations_map(term_ids).await?;
        let mut names = HashMap::new();
        for term in terms {
            let translations = translations_by_term
                .get(&term.id)
                .cloned()
                .unwrap_or_default();
            let resolved = resolve_by_locale_with_fallback(
                &translations,
                &locale,
                fallback_locale.as_deref(),
                |translation| translation.locale.as_str(),
            );
            names.insert(
                term.id,
                resolved
                    .item
                    .map(|translation| translation.name.clone())
                    .unwrap_or_else(|| term.canonical_key.clone()),
            );
        }

        Ok(names)
    }

    #[instrument(skip(self, security))]
    #[allow(clippy::too_many_arguments)]
    pub async fn resolve_term_for_module(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        kind: TaxonomyTermKind,
        module_slug: &str,
        locale: &str,
        slug_or_alias: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<Option<TaxonomyTermResponse>> {
        enforce_scope(&security, Resource::Taxonomy, Action::Read)?;

        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let module_scope = normalize_scope_value(TaxonomyScopeType::Module, Some(module_slug))?;
        let normalized_slug = normalize_term_slug(slug_or_alias);
        if normalized_slug.is_empty() {
            return Ok(None);
        }

        for (scope_type, scope_value) in [
            (TaxonomyScopeType::Module, module_scope.as_str()),
            (TaxonomyScopeType::Global, ""),
        ] {
            if let Some(term_id) = self
                .find_term_id_by_localized_slug_or_alias(
                    tenant_id,
                    kind,
                    scope_type,
                    scope_value,
                    &locale,
                    fallback_locale.as_deref(),
                    &normalized_slug,
                )
                .await?
            {
                return self
                    .get_term(
                        tenant_id,
                        security,
                        term_id,
                        &locale,
                        fallback_locale.as_deref(),
                    )
                    .await
                    .map(Some);
            }
        }

        Ok(None)
    }

    async fn find_term(
        &self,
        tenant_id: Uuid,
        term_id: Uuid,
    ) -> TaxonomyResult<taxonomy_term::Model> {
        taxonomy_term::Entity::find_by_id(term_id)
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(TaxonomyError::TermNotFound(term_id))
    }

    async fn load_translations(
        &self,
        term_id: Uuid,
    ) -> TaxonomyResult<Vec<taxonomy_term_translation::Model>> {
        Ok(taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
            .all(&self.db)
            .await?)
    }

    async fn load_aliases(&self, term_id: Uuid) -> TaxonomyResult<Vec<taxonomy_term_alias::Model>> {
        Ok(taxonomy_term_alias::Entity::find()
            .filter(taxonomy_term_alias::Column::TermId.eq(term_id))
            .all(&self.db)
            .await?)
    }

    async fn load_translations_map(
        &self,
        term_ids: &[Uuid],
    ) -> TaxonomyResult<HashMap<Uuid, Vec<taxonomy_term_translation::Model>>> {
        if term_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut map = HashMap::new();
        for translation in taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TermId.is_in(term_ids.to_vec()))
            .all(&self.db)
            .await?
        {
            map.entry(translation.term_id)
                .or_insert_with(Vec::new)
                .push(translation);
        }
        Ok(map)
    }

    #[allow(clippy::too_many_arguments)]
    async fn ensure_canonical_key_available_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        scope_type: TaxonomyScopeType,
        scope_value: &str,
        canonical_key: &str,
        exclude_term_id: Option<Uuid>,
    ) -> TaxonomyResult<()> {
        let mut select = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(kind))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .filter(taxonomy_term::Column::CanonicalKey.eq(canonical_key));
        if let Some(exclude_term_id) = exclude_term_id {
            select = select.filter(taxonomy_term::Column::Id.ne(exclude_term_id));
        }
        if select.one(txn).await?.is_some() {
            return Err(TaxonomyError::DuplicateCanonicalKey(
                canonical_key.to_string(),
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn ensure_translation_slug_available_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        scope_type: TaxonomyScopeType,
        scope_value: &str,
        locale: &str,
        slug: &str,
        exclude_term_id: Option<Uuid>,
    ) -> TaxonomyResult<()> {
        let mut select = taxonomy_term_translation::Entity::find()
            .join(
                JoinType::InnerJoin,
                taxonomy_term_translation::Relation::Term.def(),
            )
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_translation::Column::Locale.eq(locale))
            .filter(taxonomy_term_translation::Column::Slug.eq(slug))
            .filter(taxonomy_term::Column::Kind.eq(kind))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value));
        if let Some(exclude_term_id) = exclude_term_id {
            select = select.filter(taxonomy_term_translation::Column::TermId.ne(exclude_term_id));
        }
        if select.one(txn).await?.is_some() {
            return Err(TaxonomyError::DuplicateSlug(slug.to_string()));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn ensure_aliases_available_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        scope_type: TaxonomyScopeType,
        scope_value: &str,
        locale: &str,
        aliases: &[String],
        exclude_term_id: Option<Uuid>,
    ) -> TaxonomyResult<()> {
        let mut seen = HashSet::new();
        for alias in aliases {
            if !seen.insert(alias.as_str()) {
                return Err(TaxonomyError::DuplicateAlias(alias.clone()));
            }

            let mut select = taxonomy_term_alias::Entity::find()
                .join(
                    JoinType::InnerJoin,
                    taxonomy_term_alias::Relation::Term.def(),
                )
                .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_term_alias::Column::Locale.eq(locale))
                .filter(taxonomy_term_alias::Column::Slug.eq(alias))
                .filter(taxonomy_term::Column::Kind.eq(kind))
                .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
                .filter(taxonomy_term::Column::ScopeValue.eq(scope_value));
            if let Some(exclude_term_id) = exclude_term_id {
                select = select.filter(taxonomy_term_alias::Column::TermId.ne(exclude_term_id));
            }
            if select.one(txn).await?.is_some() {
                return Err(TaxonomyError::DuplicateAlias(alias.clone()));
            }
        }
        Ok(())
    }

    async fn replace_aliases_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        term_id: Uuid,
        locale: &str,
        aliases: &[String],
    ) -> TaxonomyResult<()> {
        taxonomy_term_alias::Entity::delete_many()
            .filter(taxonomy_term_alias::Column::TermId.eq(term_id))
            .filter(taxonomy_term_alias::Column::Locale.eq(locale))
            .exec(txn)
            .await?;

        let now = Utc::now();
        for alias in aliases {
            taxonomy_term_alias::ActiveModel {
                id: Set(Uuid::new_v4()),
                term_id: Set(term_id),
                tenant_id: Set(tenant_id),
                locale: Set(locale.to_string()),
                name: Set(alias.clone()),
                slug: Set(alias.clone()),
                created_at: Set(now.into()),
            }
            .insert(txn)
            .await?;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn find_term_id_by_localized_slug_or_alias(
        &self,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        scope_type: TaxonomyScopeType,
        scope_value: &str,
        locale: &str,
        fallback_locale: Option<&str>,
        slug: &str,
    ) -> TaxonomyResult<Option<Uuid>> {
        let locales = resolve_locale_candidates(locale, fallback_locale);

        for locale_candidate in locales {
            if let Some(translation) = taxonomy_term_translation::Entity::find()
                .join(
                    JoinType::InnerJoin,
                    taxonomy_term_translation::Relation::Term.def(),
                )
                .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_term_translation::Column::Locale.eq(&locale_candidate))
                .filter(taxonomy_term_translation::Column::Slug.eq(slug))
                .filter(taxonomy_term::Column::Kind.eq(kind))
                .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
                .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
                .one(&self.db)
                .await?
            {
                return Ok(Some(translation.term_id));
            }

            if let Some(alias) = taxonomy_term_alias::Entity::find()
                .join(
                    JoinType::InnerJoin,
                    taxonomy_term_alias::Relation::Term.def(),
                )
                .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_term_alias::Column::Locale.eq(&locale_candidate))
                .filter(taxonomy_term_alias::Column::Slug.eq(slug))
                .filter(taxonomy_term::Column::Kind.eq(kind))
                .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
                .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
                .one(&self.db)
                .await?
            {
                return Ok(Some(alias.term_id));
            }
        }

        Ok(None)
    }

    async fn find_term_id_for_module_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        module_scope: &str,
        locale: &str,
        slug: &str,
    ) -> TaxonomyResult<Option<Uuid>> {
        for (scope_type, scope_value) in [
            (TaxonomyScopeType::Module, module_scope),
            (TaxonomyScopeType::Global, ""),
        ] {
            if let Some(term_id) = self
                .find_term_id_by_localized_slug_or_alias_in_tx(
                    txn,
                    tenant_id,
                    kind,
                    scope_type,
                    scope_value,
                    locale,
                    Some(PLATFORM_FALLBACK_LOCALE),
                    slug,
                )
                .await?
            {
                return Ok(Some(term_id));
            }

            if let Some(term_id) = self
                .find_term_id_by_canonical_key_in_tx(
                    txn,
                    tenant_id,
                    kind,
                    scope_type,
                    scope_value,
                    slug,
                )
                .await?
            {
                return Ok(Some(term_id));
            }
        }

        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_module_term_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        module_scope: &str,
        locale: &str,
        name: &str,
        normalized_slug: &str,
    ) -> TaxonomyResult<Uuid> {
        self.ensure_canonical_key_available_in_tx(
            txn,
            tenant_id,
            kind,
            TaxonomyScopeType::Module,
            module_scope,
            normalized_slug,
            None,
        )
        .await?;
        self.ensure_translation_slug_available_in_tx(
            txn,
            tenant_id,
            kind,
            TaxonomyScopeType::Module,
            module_scope,
            locale,
            normalized_slug,
            None,
        )
        .await?;

        let now = Utc::now();
        let term_id = Uuid::new_v4();
        taxonomy_term::ActiveModel {
            id: Set(term_id),
            tenant_id: Set(tenant_id),
            kind: Set(kind),
            scope_type: Set(TaxonomyScopeType::Module),
            scope_value: Set(module_scope.to_string()),
            canonical_key: Set(normalized_slug.to_string()),
            status: Set(TaxonomyTermStatus::Active),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(txn)
        .await?;

        taxonomy_term_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            term_id: Set(term_id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.to_string()),
            name: Set(name.to_string()),
            slug: Set(normalized_slug.to_string()),
            description: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(txn)
        .await?;

        Ok(term_id)
    }

    async fn find_term_id_by_canonical_key_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        scope_type: TaxonomyScopeType,
        scope_value: &str,
        canonical_key: &str,
    ) -> TaxonomyResult<Option<Uuid>> {
        Ok(taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(kind))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .filter(taxonomy_term::Column::CanonicalKey.eq(canonical_key))
            .one(txn)
            .await?
            .map(|term| term.id))
    }

    #[allow(clippy::too_many_arguments)]
    async fn find_term_id_by_localized_slug_or_alias_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        scope_type: TaxonomyScopeType,
        scope_value: &str,
        locale: &str,
        fallback_locale: Option<&str>,
        slug: &str,
    ) -> TaxonomyResult<Option<Uuid>> {
        let locales = resolve_locale_candidates(locale, fallback_locale);

        for locale_candidate in locales {
            if let Some(translation) = taxonomy_term_translation::Entity::find()
                .join(
                    JoinType::InnerJoin,
                    taxonomy_term_translation::Relation::Term.def(),
                )
                .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_term_translation::Column::Locale.eq(&locale_candidate))
                .filter(taxonomy_term_translation::Column::Slug.eq(slug))
                .filter(taxonomy_term::Column::Kind.eq(kind))
                .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
                .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
                .one(txn)
                .await?
            {
                return Ok(Some(translation.term_id));
            }

            if let Some(alias) = taxonomy_term_alias::Entity::find()
                .join(
                    JoinType::InnerJoin,
                    taxonomy_term_alias::Relation::Term.def(),
                )
                .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_term_alias::Column::Locale.eq(&locale_candidate))
                .filter(taxonomy_term_alias::Column::Slug.eq(slug))
                .filter(taxonomy_term::Column::Kind.eq(kind))
                .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
                .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
                .one(txn)
                .await?
            {
                return Ok(Some(alias.term_id));
            }
        }

        Ok(None)
    }
}

fn build_term_response(
    term: taxonomy_term::Model,
    translations: Vec<taxonomy_term_translation::Model>,
    aliases: Vec<taxonomy_term_alias::Model>,
    locale: &str,
    fallback_locale: Option<&str>,
) -> TaxonomyTermResponse {
    let resolved =
        resolve_by_locale_with_fallback(&translations, locale, fallback_locale, |translation| {
            translation.locale.as_str()
        });
    let alias_values = resolve_aliases_for_locale(&aliases, locale, fallback_locale);

    TaxonomyTermResponse {
        id: term.id,
        tenant_id: term.tenant_id,
        kind: term.kind,
        scope_type: term.scope_type,
        scope_value: decode_scope_value(term.scope_type, &term.scope_value),
        canonical_key: term.canonical_key,
        status: term.status,
        requested_locale: locale.to_string(),
        effective_locale: resolved.effective_locale,
        available_locales: available_locales_from(&translations, |translation| {
            translation.locale.as_str()
        }),
        name: resolved
            .item
            .map(|translation| translation.name.clone())
            .unwrap_or_default(),
        slug: resolved
            .item
            .map(|translation| translation.slug.clone())
            .unwrap_or_default(),
        description: resolved
            .item
            .and_then(|translation| translation.description.clone()),
        aliases: alias_values,
        created_at: term.created_at.into(),
        updated_at: term.updated_at.into(),
    }
}

fn enforce_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
) -> TaxonomyResult<()> {
    if matches!(security.get_scope(resource, action), PermissionScope::None) {
        return Err(TaxonomyError::forbidden("Permission denied"));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> TaxonomyResult<String> {
    normalize_locale_code(locale).ok_or_else(|| TaxonomyError::validation("Invalid locale"))
}

fn validate_term_name(name: &str) -> TaxonomyResult<()> {
    if name.trim().is_empty() {
        return Err(TaxonomyError::validation("Term name cannot be empty"));
    }
    if name.chars().count() > 120 {
        return Err(TaxonomyError::validation(
            "Term name cannot exceed 120 characters",
        ));
    }
    Ok(())
}

fn validate_optional_description(description: Option<&str>) -> TaxonomyResult<()> {
    if let Some(description) = description {
        if description.chars().count() > 2_000 {
            return Err(TaxonomyError::validation(
                "Description cannot exceed 2000 characters",
            ));
        }
    }
    Ok(())
}

fn normalize_scope_value(
    scope_type: TaxonomyScopeType,
    scope_value: Option<&str>,
) -> TaxonomyResult<String> {
    match scope_type {
        TaxonomyScopeType::Global => Ok(String::new()),
        TaxonomyScopeType::Module => {
            let value = normalize_optional_scope_label(scope_value.unwrap_or_default());
            if value.is_empty() {
                return Err(TaxonomyError::validation(
                    "Module scope requires a non-empty scope_value",
                ));
            }
            Ok(value)
        }
    }
}

fn normalize_optional_scope_label(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect()
}

fn decode_scope_value(scope_type: TaxonomyScopeType, scope_value: &str) -> Option<String> {
    match scope_type {
        TaxonomyScopeType::Global => None,
        TaxonomyScopeType::Module => Some(scope_value.to_string()),
    }
}

fn normalize_term_slug(value: &str) -> String {
    slug::slugify(value)
}

fn normalize_aliases(aliases: &[String]) -> Vec<String> {
    let mut normalized = aliases
        .iter()
        .map(|alias| normalize_term_slug(alias))
        .filter(|alias| !alias.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn resolve_aliases_for_locale(
    aliases: &[taxonomy_term_alias::Model],
    locale: &str,
    fallback_locale: Option<&str>,
) -> Vec<String> {
    for candidate in resolve_locale_candidates(locale, fallback_locale) {
        let matching = aliases
            .iter()
            .filter(|alias| alias.locale == candidate)
            .map(|alias| alias.name.clone())
            .collect::<Vec<_>>();
        if !matching.is_empty() {
            return matching;
        }
    }

    Vec::new()
}

fn resolve_locale_candidates(locale: &str, fallback_locale: Option<&str>) -> Vec<String> {
    let mut candidates = vec![locale.to_string()];
    if let Some(fallback_locale) = fallback_locale {
        if fallback_locale != locale {
            candidates.push(fallback_locale.to_string());
        }
    }
    if locale != PLATFORM_FALLBACK_LOCALE && fallback_locale != Some(PLATFORM_FALLBACK_LOCALE) {
        candidates.push(PLATFORM_FALLBACK_LOCALE.to_string());
    }
    candidates
}

#[cfg(test)]
mod tests {
    use rustok_core::MigrationSource;
    use rustok_core::UserRole;
    use rustok_test_utils::db::setup_test_db;
    use sea_orm::DatabaseConnection;
    use sea_orm_migration::prelude::SchemaManager;

    use super::*;
    use crate::TaxonomyModule;

    async fn setup() -> (DatabaseConnection, TaxonomyService) {
        let db = setup_test_db().await;
        let schema_manager = SchemaManager::new(&db);
        for migration in TaxonomyModule.migrations() {
            migration
                .up(&schema_manager)
                .await
                .expect("failed to run taxonomy migration");
        }
        let service = TaxonomyService::new(db.clone());
        (db, service)
    }

    fn admin() -> SecurityContext {
        SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
    }

    #[tokio::test]
    async fn create_and_get_global_term_with_aliases() {
        let (_db, service) = setup().await;
        let tenant_id = Uuid::new_v4();

        let term_id = service
            .create_term(
                tenant_id,
                admin(),
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Global,
                    scope_value: None,
                    locale: "en".to_string(),
                    name: "Rust".to_string(),
                    slug: None,
                    canonical_key: None,
                    description: Some("Systems language".to_string()),
                    aliases: vec!["rust-lang".to_string(), "rust language".to_string()],
                },
            )
            .await
            .expect("term should be created");

        let term = service
            .get_term(tenant_id, admin(), term_id, "en", Some("ru"))
            .await
            .expect("term should load");

        assert_eq!(term.kind, TaxonomyTermKind::Tag);
        assert_eq!(term.scope_type, TaxonomyScopeType::Global);
        assert_eq!(term.scope_value, None);
        assert_eq!(term.canonical_key, "rust");
        assert_eq!(term.name, "Rust");
        assert_eq!(term.slug, "rust");
        assert_eq!(
            term.aliases,
            vec!["rust-lang".to_string(), "rust-language".to_string()]
        );
    }

    #[tokio::test]
    async fn module_and_global_terms_can_share_canonical_key() {
        let (_db, service) = setup().await;
        let tenant_id = Uuid::new_v4();
        let security = admin();

        service
            .create_term(
                tenant_id,
                security.clone(),
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Global,
                    scope_value: None,
                    locale: "en".to_string(),
                    name: "Rust".to_string(),
                    slug: None,
                    canonical_key: None,
                    description: None,
                    aliases: vec![],
                },
            )
            .await
            .expect("global term should be created");

        service
            .create_term(
                tenant_id,
                security,
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Module,
                    scope_value: Some("blog".to_string()),
                    locale: "en".to_string(),
                    name: "Rust".to_string(),
                    slug: None,
                    canonical_key: None,
                    description: None,
                    aliases: vec![],
                },
            )
            .await
            .expect("module-scoped term should be created");
    }

    #[tokio::test]
    async fn duplicate_slug_is_rejected_within_same_scope() {
        let (_db, service) = setup().await;
        let tenant_id = Uuid::new_v4();
        let security = admin();

        service
            .create_term(
                tenant_id,
                security.clone(),
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Module,
                    scope_value: Some("forum".to_string()),
                    locale: "en".to_string(),
                    name: "Rust".to_string(),
                    slug: Some("systems".to_string()),
                    canonical_key: None,
                    description: None,
                    aliases: vec![],
                },
            )
            .await
            .expect("first term should be created");

        let error = service
            .create_term(
                tenant_id,
                security,
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Module,
                    scope_value: Some("forum".to_string()),
                    locale: "en".to_string(),
                    name: "Zig".to_string(),
                    slug: Some("systems".to_string()),
                    canonical_key: Some("zig".to_string()),
                    description: None,
                    aliases: vec![],
                },
            )
            .await
            .expect_err("duplicate localized slug should be rejected");

        assert!(matches!(error, TaxonomyError::DuplicateSlug(slug) if slug == "systems"));
    }

    #[tokio::test]
    async fn get_term_uses_translation_fallback() {
        let (_db, service) = setup().await;
        let tenant_id = Uuid::new_v4();
        let security = admin();

        let term_id = service
            .create_term(
                tenant_id,
                security.clone(),
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Global,
                    scope_value: None,
                    locale: "en".to_string(),
                    name: "Rust".to_string(),
                    slug: None,
                    canonical_key: None,
                    description: Some("Systems language".to_string()),
                    aliases: vec!["rust language".to_string()],
                },
            )
            .await
            .expect("term should be created");

        service
            .update_term(
                tenant_id,
                term_id,
                security.clone(),
                UpdateTaxonomyTermInput {
                    locale: "ru".to_string(),
                    name: Some("Раст".to_string()),
                    slug: Some("rast".to_string()),
                    description: Some("Язык системного программирования".to_string()),
                    status: None,
                    aliases: Some(vec!["rust-ru".to_string()]),
                },
            )
            .await
            .expect("ru translation should be added");

        let term = service
            .get_term(tenant_id, security, term_id, "de", Some("ru"))
            .await
            .expect("term should resolve with fallback");

        assert_eq!(term.requested_locale, "de");
        assert_eq!(term.effective_locale, "ru");
        assert_eq!(term.name, "Раст");
        assert_eq!(term.slug, "rast");
        assert_eq!(term.aliases, vec!["rust-ru".to_string()]);
        assert_eq!(
            term.available_locales,
            vec!["en".to_string(), "ru".to_string()]
        );
    }

    #[tokio::test]
    async fn resolve_term_for_module_prefers_module_scope_before_global() {
        let (_db, service) = setup().await;
        let tenant_id = Uuid::new_v4();
        let security = admin();

        let global_term_id = service
            .create_term(
                tenant_id,
                security.clone(),
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Global,
                    scope_value: None,
                    locale: "en".to_string(),
                    name: "Rust".to_string(),
                    slug: Some("rust".to_string()),
                    canonical_key: None,
                    description: None,
                    aliases: vec![],
                },
            )
            .await
            .expect("global term should be created");

        let module_term_id = service
            .create_term(
                tenant_id,
                security.clone(),
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type: TaxonomyScopeType::Module,
                    scope_value: Some("blog".to_string()),
                    locale: "en".to_string(),
                    name: "Rust".to_string(),
                    slug: Some("rust".to_string()),
                    canonical_key: None,
                    description: None,
                    aliases: vec![],
                },
            )
            .await
            .expect("module term should be created");

        let resolved = service
            .resolve_term_for_module(
                tenant_id,
                security,
                TaxonomyTermKind::Tag,
                "blog",
                "en",
                "rust",
                Some("en"),
            )
            .await
            .expect("resolve should succeed")
            .expect("term should resolve");

        assert_eq!(resolved.id, module_term_id);
        assert_ne!(resolved.id, global_term_id);
    }
}
