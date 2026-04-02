//! User-configurable composite blocks with IL generation and deployment integration.
//!
//! This crate provides a [`ConfigurableBlock`] trait for blocks that:
//! - Declare a configuration schema (editable parameters with types and defaults)
//! - Belong to a category for sub-menu grouping in the UI palette
//! - Lower themselves to DAG IL ([`dag_core::op::Op`] sequences)
//! - Declare pubsub topics and hardware deployment channel names
//!
//! # Demo block
//!
//! The [`blocks::pid`] module provides a PID controller that subscribes to
//! setpoint/feedback topics, computes P+I+D terms, clamps output, and publishes
//! the result — all as a single configurable block that lowers to ~20 DAG ops.

pub mod blocks;
pub mod codegen;
pub mod lower;
pub mod registry;
pub mod schema;
