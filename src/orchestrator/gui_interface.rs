//! GUI-friendly interface for the orchestrator.
//!
//! This module provides types and functions to make orchestrator state
//! easily accessible to GUI applications like Bevy. It separates GUI
//! concerns from core orchestrator logic.

use serde::{Deserialize, Serialize};
use common_game::utils::ID;
use crate::orchestrator::state::SystemState;
use crate::orchestrator::galaxy_ai::GalaxyAI;
use std::fmt::Display;

/// Events that flow FROM the orchestrator TO the GUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuiEvent {
    PlanetAdded(ID),
    PlanetRemoved(ID),
    PlanetStateUpdated(ID),
    ExplorerAdded(ID, ID), // explorer_id, planet_id
    ExplorerRemoved(ID),
    ExplorerMoved(ID, ID, ID), // explorer_id, from_planet, to_planet
    ExplorerArrived(ID, ID),   // explorer_id, planet_id
    ExplorerMoveRejected(ID, ID, String), // explorer_id, planet_id, reason
    ExplorerLocationConfirmed(ID, ID),    // explorer_id, planet_id
    ExplorerBagUpdated(ID),
    ResourcesDiscovered(ID),
    CombinationsDiscovered(ID),
    ResourceGenerated(ID, bool), // explorer_id, success
    ResourceCombined(ID, bool),  // explorer_id, success
    AsteroidSent(ID),
    AsteroidHit(ID, bool),       // planet_id, defended
    SunraySent(ID),
    SunrayReceived(ID),
    /// Galaxy AI selected a new set of (up to 3) target planets.
    GalaxyAiTargetsSelected(Vec<ID>),
    /// Galaxy AI fired an asteroid at this planet.
    GalaxyAiAsteroidLaunched(ID),
    /// Galaxy AI entered a sunray-only cooldown for this many time intervals.
    GalaxyAiCooldownStarted { intervals: u32 },
    StateUpdate(GuiState),
}

/// Commands that flow FROM the GUI TO the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuiCommand {
    SendAsteroid { planet_id: ID },
    SendSunray { planet_id: ID },
    TogglePlanetAI { planet_id: ID, enabled: bool },
    ToggleExplorerAI { explorer_id: ID, enabled: bool },
    PauseSimulation,
    ResumeSimulation,
    SetSimulationCycleLengthInMillis { millis: u64 },
    EnableGalaxyAI,
    DisableGalaxyAI,
}

/// GUI representation of the complete system state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiState {
    pub planets: Vec<GuiPlanet>,
    pub explorers: Vec<GuiExplorer>,
    pub game_stats: GuiGameStats,
    pub galaxy_ai: GuiGalaxyAiState,
    pub simulation_time: u64,
}

/// GUI representation of a single planet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiPlanet {
    pub id: ID,
    pub planet_type: String,
    pub energy_level: f32,
    pub charged_cells: usize,
    pub has_rocket: bool,
    pub ai_active: bool,
    pub x: f32,
    pub y: f32,
    pub neighbors: Vec<ID>,
    pub explorers: Vec<ID>,
    pub available_resources: Vec<String>,
}

/// GUI representation of a single explorer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiExplorer {
    pub id: ID,
    pub current_planet: ID,
    pub health: f32,
    pub bag_count: usize,
    pub ai_active: bool,
    pub mode: String,
    pub x: f32,
    pub y: f32,
}

/// GUI representation of game-wide statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiGameStats {
    pub asteroids_sent: u32,
    pub sunrays_sent: u32,
    pub planets_destroyed: u32,
    pub explorers_killed: u32,
    pub resources_generated: u32,
}

/// GUI representation of the Galaxy AI's current asteroid campaign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiGalaxyAiState {
    pub enabled: bool,
    /// Up to 3 planets currently designated as targets.
    pub target_set: Vec<ID>,
    /// Planet an in-flight asteroid is aimed at, if any.
    pub pending_target: Option<ID>,
    /// How many of the 2 attempts against the current set have resolved.
    pub attempts_made: u8,
    /// Time intervals left in the current sunray-only cooldown (0 if not cooling down).
    pub cooldown_intervals_remaining: u32,
}

impl GuiState {
    /// Creates a GUI state snapshot from the system state and galaxy AI.
    #[must_use]
    pub fn from_system_state(system_state: &SystemState, galaxy_ai: &GalaxyAI) -> Self {
        let planets: Vec<GuiPlanet> = system_state
            .get_alive_planets_sorted()
            .into_iter()
            .enumerate()
            .map(|(i, planet_id)| {
                let stats = system_state
                    .get_planet_stats(planet_id)
                    .cloned()
                    .unwrap_or_default();

                let planet_count = system_state.get_alive_planets_sorted().len();
                let radius = 300.0;
                let angle = (i as f32) * 2.0 * std::f32::consts::PI / (planet_count as f32).max(1.0);

                GuiPlanet {
                    id: planet_id,
                    planet_type: stats.planet_type.to_string(),
                    energy_level: stats.energy_level,
                    charged_cells: stats.charged_cells,
                    has_rocket: stats.has_rocket,
                    ai_active: true, // TODO: Get actual AI state from planet
                    x: radius * angle.cos(),
                    y: radius * angle.sin(),
                    neighbors: system_state.get_neighbors(planet_id),
                    explorers: system_state.get_explorers_on_planet(planet_id),
                    available_resources: stats.available_resources,
                }
            })
            .collect();

        let explorers: Vec<GuiExplorer> = system_state
            .explorer_locations()
            .iter()
            .map(|(&explorer_id, &planet_id)| {
                let stats = system_state
                    .explorer_stats(explorer_id)
                    .cloned()
                    .unwrap_or_default();

                let planet_pos = planets
                    .iter()
                    .find(|p| p.id == planet_id)
                    .map(|p| (p.x, p.y))
                    .unwrap_or((0.0, 0.0));

                let offset = 20.0;
                let angle = (explorer_id as f32) * 0.5;

                GuiExplorer {
                    id: explorer_id,
                    current_planet: planet_id,
                    health: stats.health,
                    bag_count: stats.bag_count,
                    ai_active: stats.ai_active,
                    mode: format!("{}", stats.mode),
                    x: planet_pos.0 + offset * angle.cos(),
                    y: planet_pos.1 + offset * angle.sin(),
                }
            })
            .collect();

        let game_stats = system_state.game_stats();

        Self {
            planets,
            explorers,
            game_stats: GuiGameStats {
                asteroids_sent: game_stats.asteroids_sent,
                sunrays_sent: game_stats.sunrays_sent,
                planets_destroyed: game_stats.planets_destroyed,
                explorers_killed: game_stats.explorers_killed,
                resources_generated: game_stats.resources_generated,
            },
            galaxy_ai: GuiGalaxyAiState {
                enabled: galaxy_ai.is_enabled(),
                target_set: galaxy_ai.get_target_set().to_vec(),
                pending_target: galaxy_ai.get_pending_target(),
                attempts_made: galaxy_ai.get_attempts_made(),
                cooldown_intervals_remaining: galaxy_ai.get_cooldown_intervals_remaining(),
            },
            simulation_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as u64,
        }
    }

    /// Gets the current mood emoji based on recent events.
    #[must_use]
    pub fn get_current_mood(&self) -> &'static str {
        if self.game_stats.asteroids_sent > 0 && self.game_stats.sunrays_sent == 0 {
            "😠"
        } else if self.game_stats.sunrays_sent > 0 && self.game_stats.asteroids_sent == 0 {
            "😊"
        } else if self.game_stats.asteroids_sent > 0 && self.game_stats.sunrays_sent > 0 {
            "🎲"
        } else {
            "😐"
        }
    }
}