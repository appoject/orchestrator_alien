//! Main orchestrator module.
//!
//! This module contains the internal components used to coordinate the space
//! simulation, including planet management, explorer control, message routing,
//! and optional GUI integration.
//!
//! Only a small, well-defined public API is exposed. All other modules are
//! considered internal implementation details.

pub mod routing;
pub mod planet_control;
pub mod explorer_control;
pub mod gui_interface;

// Rendi pubblici i moduli interni che vuoi usare fuori dal crate
pub mod state;
pub mod orchestrator;
pub mod galaxy_structure;
pub mod galaxy_ai;

/// Main orchestrator entry point.
pub use orchestrator::Orchestrator;
/// Snapshot of the orchestrator internal state.
pub use state::SystemState;
/// Galaxy topology structure.
pub use galaxy_structure::GalaxyStructure;

// Re-export other public types
pub use galaxy_ai::{GalaxyAI, GalaxyAction};
pub use gui_interface::{GuiState, GuiEvent, GuiCommand};

// Internal use only
pub(crate) use planet_control::handle_planet_msg;
pub(crate) use explorer_control::handle_explorer_msg;
pub(crate) use routing::handle_travel_request;