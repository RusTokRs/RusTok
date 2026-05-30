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
}
