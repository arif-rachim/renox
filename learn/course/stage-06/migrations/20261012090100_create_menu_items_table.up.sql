CREATE TABLE "menu_items" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "category_id" INTEGER NOT NULL REFERENCES "categories" ("id"),
    "user_id" INTEGER NOT NULL REFERENCES "users" ("id"),
    "name" TEXT NOT NULL,
    "description" TEXT,
    "price" INTEGER NOT NULL,
    "available" INTEGER NOT NULL DEFAULT true,
    "stock" INTEGER,
    "created_at" TEXT,
    "updated_at" TEXT
);
CREATE UNIQUE INDEX "menu_items_name_unique" ON "menu_items" ("name");
CREATE INDEX "menu_items_category_id_index" ON "menu_items" ("category_id");
CREATE INDEX "menu_items_user_id_index" ON "menu_items" ("user_id");
