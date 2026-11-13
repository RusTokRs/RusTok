mod materialize;
mod model;
mod validation;

pub use materialize::*;
pub use model::*;
pub(crate) use validation::decode_form;
pub use validation::*;

/// The form materializer leaves conflicting or invalid interaction contracts untouched.
/// Identity projections use the same rule when deciding whether an authored HTML id is replaced.
pub(crate) fn materializable_form(component: &crate::ComponentObject) -> Option<ComponentForm> {
    if component.extensions.contains_key(FLY_ACTION_FIELD)
        || component
            .extensions
            .contains_key(crate::FLY_PAGE_LINK_FIELD)
    {
        return None;
    }
    component
        .extensions
        .get(FLY_FORM_FIELD)
        .and_then(|raw| decode_form(raw.clone()).ok())
}

#[cfg(test)]
use crate::ValidationSeverity;
#[cfg(test)]
mod tests;
