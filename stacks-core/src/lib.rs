//! stacks-core: the recipe data model and SQLite system of record.
//!
//! The model is the LLM-ergonomics contract: every type derives
//! [`serde`] and [`schemars::JsonSchema`], so JSON Schemas served by the
//! API are generated from these definitions rather than maintained by hand.

pub mod bonafides;
pub mod error;
pub mod library;
pub mod materials;
pub mod model;
pub mod quantity;
pub mod store;

pub use error::{ModelError, StoreError};
pub use model::*;
pub use quantity::{Operator, Quantity, Unit};
pub use store::{RecipeBundle, StepBundle, Store};
