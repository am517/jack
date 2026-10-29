//! Shared library for all windups. One module per concern.
pub mod conflicts;
pub mod db;
pub mod names;
pub mod paths;
pub mod prune;
pub mod settings;
pub mod unresolved;
pub mod vectors;

pub use settings::Settings;
