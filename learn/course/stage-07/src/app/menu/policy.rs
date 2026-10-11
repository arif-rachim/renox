use renox::prelude::*;

use super::model::MenuItem;

// [learn:menu.policy]
/// Who may do what to a menu item.
impl Policy for MenuItem {
    fn allows(&self, user: &User, ability: &str) -> bool {
        match ability {
            "view" => true,
            // Only the person who added an item changes its stock.
            "update" => self.user_id == user.id,
            _ => false,
        }
    }
}
// [/learn:menu.policy]
