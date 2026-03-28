//! DAG runtime: lightweight execution engine for deployed expression DAGs.
//!
//! Provides the runtime infrastructure for evaluating [`dag_core`] DAGs on
//! embedded targets, with channel I/O, pub/sub messaging, and HTTP serving
//! for the DAG editor frontend.
//!
//! # Modules
//!
//! - [`executor`] — Tick-based DAG executor with configurable frequency.
//! - [`channels`] — Channel map for hardware I/O (ADC reads, PWM writes).
//! - [`pubsub`] — In-memory pub/sub topic store for inter-node communication.
//! - [`http`] — Minimal HTTP request parser for the embedded API server.
//! - [`generated`] — Generated frontend assets (HTML, JS, CSS) served from flash.
//! - [`serve`] — HTTP response builder for the DAG editor web UI.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod channels;
pub mod executor;
pub mod generated;
pub mod http;
pub mod pubsub;
pub mod serve;
