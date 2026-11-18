//! server-guard library: configuration, event model, SSH log parsing, and agent runtime.

pub mod agent;
pub mod config;
pub mod error;
pub mod events;
pub mod i18n;
pub mod monitors;
pub mod services;
pub mod utils;

pub use i18n::Msg;
