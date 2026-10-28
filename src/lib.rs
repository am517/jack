//! Shared library for all windups. One module per concern.
pub mod db;
pub mod paths;
pub mod prune;
pub mod settings;

pub use settings::Settings;
