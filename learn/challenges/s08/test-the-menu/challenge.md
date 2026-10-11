---
title: Test the menu page
stage: 8
files: tests/menu_index.rs
test: challenge_test_the_menu
minutes: 20
summary: Write two tests for the menu page that the app does not have yet.
---
The menu page has tests for what it lists, and for the items it hides. Two behaviours have none. In the file `tests/menu_index.rs` write two tests, each marked `#[renox::test]`:

1. **An empty menu says so.** Boot the app without seeding it, `GET /menu`, and check the page shows "Nothing on the menu yet".
2. **Items come in order within a category.** Seed the app, `GET /menu`, and check that "Cappuccino" comes before "Flat white" in the page. Both are coffees, listed by name. The response's `text()` gives you the page as a `String`, and `find` gives the position of a word in it.

The test below does not test the menu. It looks at your file: it checks that you wrote the two tests, and it runs them. If your tests fail, it fails.
