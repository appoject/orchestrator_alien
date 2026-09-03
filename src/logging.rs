//! Logging setup and configuration for the orchestrator.
//!
//! This module provides a simple way to initialize logging that integrates
//! with the common_game::logging system while providing orchestrator-specific
//! logging capabilities.

use log::LevelFilter;

/// Initialize the logging system for the orchestrator.
///
/// This sets up the log crate with the specified log level and ensures
/// compatibility with the common-game logging system.
///
/// # Examples
///
/// ```no_run
///use Orchestrator::logging;
///use log::LevelFilter;
///logging::init(LevelFilter::Info);
/// ```
///
/// # Panics
///
/// This function will panic if logging has already been initialized.
pub fn init(level: LevelFilter) {
    env_logger::Builder::new()
        .filter_level(level)
        .filter_module("orchestrator", level)
        .filter_module("common_game", level)
        .format_timestamp_micros()
        .format_module_path(false)
        .init();

    log::info!("Orchestrator logging initialized at level {:?}", level);
}
