//! Read-only Discord reconnaissance modules.
//!
//! Every module uses **public or authorized API surfaces only**: keyless
//! public endpoints (invites, guild widgets, webhook metadata) and
//! official bot-token endpoints (user profiles). No selfbots, no user-token
//! automation, no message scraping, no mass actions.

pub mod api;
pub mod full;
pub mod guild;
pub mod invite;
pub mod snowflake;
pub mod user;
pub mod webhook;
