//! Audio runtime primitives for Stellatune playback.
//!
//! This crate provides the runtime-facing audio engine used by the backend.
//! It exposes configuration models, engine control surfaces, pipeline assembly
//! abstractions, and typed runtime errors.
//!
//! # Architecture
//!
//! The public surface is organized into four modules:
//! - [`config`]: user-visible runtime configuration and event types.
//! - [`engine`]: control actor startup and the [`engine::EngineHandle`] API.
//! - [`error`]: typed error enums for engine and decode worker operations.
//! - [`pipeline`]: strongly typed planning, stage factories, and session assembly.
//!
//! Internal modules such as `infra` and `workers` are intentionally private.
//!
//! For a maintainer-focused architecture walkthrough, see
//! `docs/stellatune-audio-architecture.md` in the repository root.
//!
//! # Error Model
//!
//! Engine entry points return [`error::EngineError`]. Decode-layer failures are
//! represented as [`error::DecodeError`] and are propagated through
//! `EngineError::Decode`.
//!
//! # Examples
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use stellatune_audio::engine::start_engine;
//! use stellatune_audio::pipeline::assembly::PipelineFactory;
//!
//! # fn factory() -> Arc<dyn PipelineFactory> { todo!() }
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = start_engine(factory())?;
//! let _snapshot = engine.snapshot().await?;
//! # Ok(())
//! # }
//! ```
#![deny(clippy::wildcard_imports)]

pub mod config;
pub mod engine;
pub mod error;
pub mod gapless;
mod infra;
pub mod pipeline;
mod workers;
