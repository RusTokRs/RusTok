mod graphql_adapter;
mod native_server_adapter;

use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};

use crate::model::{
    TranslationAdminOperation, TranslationAdminResponse, TranslationAdminTransportContext,
};

pub type TransportError = UiTransportError;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

/// Executes the selected build-profile transport without protocol fallback.
pub(crate) async fn execute(
    context: TranslationAdminTransportContext,
    operation: TranslationAdminOperation,
) -> Result<TranslationAdminResponse, TransportError> {
    let native_operation = operation.clone();
    execute_selected_transport(
        "translation_admin",
        selected_transport_path(),
        move || native_server_adapter::execute_translation_native(native_operation),
        move || graphql_adapter::execute(context, operation),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_selects_graphql() {
        #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
