//! The panel's resources: one file per model, each declaring its grid
//! columns, its form fields and its policy once.

mod categories;
mod customers;
mod products;

pub use categories::{Category, CategoryForm, CategoryResource};
pub use customers::{Customer, CustomerForm, CustomerResource};
pub use products::{Product, ProductForm, ProductResource};
