//! A bounded household ledger, independent of its HTTP and ESP32 adapters.
pub mod api;
pub mod auth;
pub mod board;
mod client;
pub mod database_diagnostics;
pub mod diagnostics;
pub mod domain;
#[cfg(all(
    feature = "cobol-core",
    not(any(target_os = "windows", target_os = "espidf"))
))]
compile_error!("cobol-core supports Windows DLLs and experimental ESP-IDF static linkage");
#[cfg(feature = "cobol-core")]
pub mod cobol;
pub mod events;
pub mod journal;
mod json;
pub mod lending;
pub mod loans;
pub mod money;
pub mod offers;
pub mod web;

pub mod forex;

pub mod lotto;

pub mod fulfillment;
pub mod incidents;

pub mod commerce;
pub mod ledger;

pub mod board_status;

pub mod cache;

pub mod screen;

pub mod server;

pub mod minicloud;

#[cfg(test)]
#[path = "tests/bank_integrity.rs"]
mod bank_integrity;
