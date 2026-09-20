//! A bounded household ledger, independent of its HTTP and ESP32 adapters.
pub mod api;
pub mod auth;
mod client;
pub mod diagnostics;
pub mod domain;
pub mod journal;
mod json;
pub mod offers;
pub mod web;

pub mod forex;
