#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).

//! Runtime scaffolding for plugin control-plane lifecycle.
//!
//! This module owns actor/handle/model/lease management, while instance data-plane
//! execution is expected to live in business-side worker threads.

pub mod actor;
pub mod backend_control;
pub mod handle;
pub mod instance_registry;
pub mod introspection;
pub mod messages;
pub mod model;
pub mod registry;
pub mod update;
pub mod worker_controller;
pub mod worker_endpoint;
