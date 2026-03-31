//! Expression DAG core: a lightweight, `no_std`-compatible dataflow graph.
//!
//! Provides a topologically ordered DAG of arithmetic and I/O operations
//! that can be evaluated in a single forward pass. Designed for embedded
//! deployment on microcontrollers via CBOR serialization.
//!
//! # Modules
//!
//! - [`op`] — `Op` enum (Const, Input, Output, Add, Mul, Sub, Div, Pow, Neg,
//!   Relu, Subscribe, Publish) and the `Dag` container with topological validation.
//! - [`builder`] — Fluent builder API for constructing DAGs programmatically.
//! - [`eval`] — Single-pass evaluator with `Channels` and `PubSubReader` traits.
//! - [`cbor`] — Compact CBOR encode/decode via `minicbor` (typically <200 bytes
//!   for a 32-node graph).
//! - [`templates`] — Pre-built DAG templates (ADC→gain→PWM, PID loops, etc.).
//!
//! # Example
//!
//! ```rust
//! use dag_core::op::Dag;
//! use dag_core::eval::{NullChannels, NullPubSub};
//!
//! let mut dag = Dag::new();
//! let a = dag.constant(3.0).unwrap();
//! let b = dag.constant(4.0).unwrap();
//! let sum = dag.add(a, b).unwrap();
//! dag.publish("result", sum).unwrap();
//!
//! let mut values = vec![0.0; dag.len()];
//! let result = dag.evaluate(&NullChannels, &NullPubSub, &mut values);
//! assert_eq!(result.publishes[0].1, 7.0);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod builder;
pub mod cbor;
pub mod eval;
pub mod op;
pub mod templates;

#[cfg(feature = "json")]
pub mod json;
