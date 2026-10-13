---
title: Names need three letters
stage: 5
files: src/app/menu/model.rs
test: challenge_name_min_length
minutes: 10
summary: Make the new-item form refuse a name shorter than 3 characters.
---
Someone added a menu item called `A`. A name needs to be at least 3 characters long. The rules of the form live next to the fields, in the `#[form(validate(...))]` attribute of `MenuItem`. Add the rule `min = 3` to the name's rules.

For a text field, `min` means the number of characters. You do not need to change a handler or a view: the form already shows the errors of every rule next to the field.
