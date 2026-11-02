/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Selected dictionary transport. An uncertain native result never falls through to GraphQL.

use crate::core::attribute_lifecycle::{
    AttributeWriteScope, valid_input, valid_read_request, valid_result, valid_workspace,
};
use crate::core::write_journal::{JournalScope, valid_uuid};
use crate::model::{
    AttributeDefinitionOutcome, AttributeLifecycleRequest, AttributeLifecycleWorkspace,
    UpdateAttributeDefinitionDraft,
};

pub(crate) async fn read(
    _token: Option<String>,
    _tenant: Option<String>,
    tenant_id: String,
    actor_id: String,
    locale: String,
    request: AttributeLifecycleRequest,
) -> Result<AttributeLifecycleWorkspace, ()> {
    if !valid_read_request(&request) {
        return Err(());
    }
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    let result = super::attribute_lifecycle_native::read_definition_native(
        tenant_id,
        actor_id,
        locale,
        request.clone(),
    )
    .await
    .map_err(|_| ())?;
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    let result = {
        if !super::authenticated_actor_scope_matches(
            _token.clone(),
            _tenant.clone(),
            &tenant_id,
            &actor_id,
        )
        .await
        {
            return Err(());
        }
        super::attribute_lifecycle_graphql::read(
            _token,
            _tenant,
            tenant_id,
            locale,
            request.clone(),
        )
        .await?
    };
    if valid_workspace(&result, &request) {
        Ok(result)
    } else {
        Err(())
    }
}

pub(crate) async fn update(
    _token: Option<String>,
    _tenant: Option<String>,
    scope: AttributeWriteScope,
    key: String,
    input: UpdateAttributeDefinitionDraft,
) -> AttributeDefinitionOutcome {
    if !scope.valid()
        || !valid_uuid(&key)
        || !valid_input(&input)
        || input.attribute_id != scope.attribute_id
        || input.locale != scope.locale
    {
        return AttributeDefinitionOutcome::Unknown;
    }
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    let outcome = super::attribute_lifecycle_native::update_definition_native(
        scope.tenant_id,
        scope.actor_id,
        key,
        input.clone(),
    )
    .await
    .unwrap_or(AttributeDefinitionOutcome::Unknown);
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    let outcome = {
        if !super::authenticated_actor_scope_matches(
            _token.clone(),
            _tenant.clone(),
            &scope.tenant_id,
            &scope.actor_id,
        )
        .await
        {
            return AttributeDefinitionOutcome::Unknown;
        }
        super::attribute_lifecycle_graphql::update(_token, _tenant, key, input.clone()).await
    };
    match outcome {
        AttributeDefinitionOutcome::Decision { ref result } if !valid_result(&input, result) => {
            AttributeDefinitionOutcome::Unknown
        }
        outcome => outcome,
    }
}
