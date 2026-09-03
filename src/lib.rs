
//! # Orchestrator Library - Luna4 Group
//!
//! This crate provides the **Orchestrator implementation** for the AP galaxy
//! simulation game.
//!
//! It is an application-level crate, not a shared common library.
//! Its public API is intentionally small and limited to the [`Orchestrator`] type.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod logging;
pub mod orchestrator;

//mod gui;

/// Main entry point of the orchestrator system.
pub use orchestrator::Orchestrator;

/// Re-export SystemState so it can be used externally
pub use orchestrator::state::SystemState;

/// Re-export GalaxyStructure so it can be used externally
pub use orchestrator::galaxy_structure::GalaxyStructure;

// Re-export the main orchestrator type and public APIs
pub use crate::orchestrator::gui_interface::GuiState;
pub use crate::orchestrator::gui_interface::GuiEvent;
pub use crate::orchestrator::gui_interface::GuiCommand;
