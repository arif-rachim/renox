---
title: Allergens on the menu
stage: 1
files: src/app/menu/model.rs
test: challenge_add_allergens
minutes: 15
summary: Add a nullable allergens column to menu_items with a model field and a migration.
---
A customer asks what is in the croissant. Give every menu item an optional note about its allergens, like "milk, gluten".

1. Add the field `allergens: Option<String>` to `MenuItem`. `Option` makes the column nullable: an item without a note stays valid.
2. Let Renox write the migration from the difference between the model and the tables:

   ```bash
   rnx make:migration --auto add_allergens_to_menu_items --yes
   ```

   It writes an `.up.sql` and a `.down.sql` file in `migrations/`. Rename them to start with `20261012100000`, so they sort after the migrations you already have.
3. Read the two files. The up file adds the column, and the down file must remove it, so that a rollback works.

The new migration is not in the list of files below: it is a file you generate. The solution shows what it looks like.
