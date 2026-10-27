#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartLineItemQuantityCommand {
    Remove,
    Update { next_quantity: i32 },
}

pub fn decrement_quantity_command(current_quantity: i32) -> CartLineItemQuantityCommand {
    if current_quantity <= 1 {
        CartLineItemQuantityCommand::Remove
    } else {
        CartLineItemQuantityCommand::Update {
            next_quantity: current_quantity - 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutHandoffFormInput {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub address: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckoutHandoffValidationError {
    NameRequired,
    EmailRequired,
    PhoneRequired,
    AddressRequired,
}

impl CheckoutHandoffValidationError {
    pub fn default_message(&self) -> &'static str {
        match self {
            Self::NameRequired => "Full name is required",
            Self::EmailRequired => "Email address is required",
            Self::PhoneRequired => "Phone number is required",
            Self::AddressRequired => "Address is required",
        }
    }
}

pub fn validate_checkout_handoff_form(
    input: &CheckoutHandoffFormInput,
) -> Result<(), CheckoutHandoffValidationError> {
    if input.name.trim().is_empty() {
        return Err(CheckoutHandoffValidationError::NameRequired);
    }
    if input.email.trim().is_empty() {
        return Err(CheckoutHandoffValidationError::EmailRequired);
    }
    if input.phone.trim().is_empty() {
        return Err(CheckoutHandoffValidationError::PhoneRequired);
    }
    if input.address.trim().is_empty() {
        return Err(CheckoutHandoffValidationError::AddressRequired);
    }
    Ok(())
}

pub fn generate_handoff_order_reference(cart_id: &str) -> String {
    let trimmed = cart_id.trim();
    let prefix: String = trimmed.chars().take(8).collect();
    format!("ord-{prefix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrement_quantity_command_keeps_write_policy_out_of_ui() {
        assert_eq!(
            decrement_quantity_command(0),
            CartLineItemQuantityCommand::Remove
        );
        assert_eq!(
            decrement_quantity_command(1),
            CartLineItemQuantityCommand::Remove
        );
        assert_eq!(
            decrement_quantity_command(3),
            CartLineItemQuantityCommand::Update { next_quantity: 2 }
        );
    }

    #[test]
    fn checkout_handoff_validation_enforces_contact_fields() {
        let valid = CheckoutHandoffFormInput {
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            phone: "+1234567890".to_string(),
            address: "123 Main St".to_string(),
        };
        assert_eq!(validate_checkout_handoff_form(&valid), Ok(()));

        let empty_name = CheckoutHandoffFormInput {
            name: "   ".to_string(),
            ..valid.clone()
        };
        assert_eq!(
            validate_checkout_handoff_form(&empty_name),
            Err(CheckoutHandoffValidationError::NameRequired)
        );

        let empty_email = CheckoutHandoffFormInput {
            email: " ".to_string(),
            ..valid.clone()
        };
        assert_eq!(
            validate_checkout_handoff_form(&empty_email),
            Err(CheckoutHandoffValidationError::EmailRequired)
        );

        let empty_phone = CheckoutHandoffFormInput {
            phone: "".to_string(),
            ..valid.clone()
        };
        assert_eq!(
            validate_checkout_handoff_form(&empty_phone),
            Err(CheckoutHandoffValidationError::PhoneRequired)
        );

        let empty_address = CheckoutHandoffFormInput {
            address: " \t ".to_string(),
            ..valid
        };
        assert_eq!(
            validate_checkout_handoff_form(&empty_address),
            Err(CheckoutHandoffValidationError::AddressRequired)
        );
    }

    #[test]
    fn handoff_order_reference_generation_is_safe_with_unicode_and_short_inputs() {
        assert_eq!(generate_handoff_order_reference(""), "ord-");
        assert_eq!(generate_handoff_order_reference("  abc  "), "ord-abc");
        assert_eq!(
            generate_handoff_order_reference("550e8400-e29b-41d4-a716-446655440000"),
            "ord-550e8400"
        );
        // UTF-8 safety check: Cyrillic characters where multi-byte boundaries must not panic
        assert_eq!(generate_handoff_order_reference("корзина-12345"), "ord-корзина-");
    }
}
