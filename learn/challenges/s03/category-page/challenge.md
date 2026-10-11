---
title: A page for each category
stage: 3
files: src/app/menu/mod.rs, resources/views/menu/category.html
test: challenge_category_page
minutes: 25
summary: Add /categories/{category}, a page that lists the available items of one category in a table.
---
A customer who likes tea wants to see only the teas. Add the page `GET /categories/{category}`, where `{category}` is the category's id.

It shows the category's name as the page title and its **available** items in an `<rx-table>`, like the menu page does. An id that is not a category answers 404.

Follow the same steps as for the item page: a route with a name (`categories.show`), a type for the page's data that you register with `app.view_data`, a handler that reads the rows, and a view. The view file exists below with a note and no content yet.
