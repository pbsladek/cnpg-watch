//! Pure domain modules and explicit I/O entry points for cnpg-watch.
pub mod cli;
pub mod config;
pub mod support;
pub mod terminal;

pub const SCHEMA_VERSION: u32 = 1;
pub mod adapters;
pub mod app;
pub mod budget;
pub mod capabilities;
pub mod collectors;
pub mod doctor;
pub mod kubernetes;
pub mod model;
pub mod probes;
pub mod queue;
pub mod reducer;
pub mod scheduler;
pub mod setup;
pub mod time;
pub mod ui;

pub mod diagnostics;
pub mod evidence;
pub mod health;
pub mod logs;
pub mod navigation;
pub mod runtime;
pub mod screens;

pub mod auxiliary;
pub mod operational;
pub mod pooler;

pub mod cli_docs;
