use renox::prelude::*;

use super::model::Product;

impl Policy for Product {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "update" | "delete" | "restore" => self.user_id == user.id,
            _ => false,
        }
    }
}
