//! OpenSubsonic API client module.

pub mod api;
pub mod auth;
pub mod models;

pub use api::SubsonicClient;
pub use auth::Auth;
