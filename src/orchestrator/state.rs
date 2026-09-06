//! System state management for the orchestrator.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use common_game::utils::ID;

use super::galaxy_structure::GalaxyStructure;

/// Overall game state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Running,
    Paused,
    Ended,
}

/// Planet types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanetType {
    A, B, C, D, Unknown,
}

impl std::fmt::Display for PlanetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::A => write!(f, "A"),
            Self::B => write!(f, "B"),
            Self::C => write!(f, "C"),
            Self::D => write!(f, "D"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Explorer modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorerMode {
    Auto,
    Manual,
}

impl Default for ExplorerMode {
    fn default() -> Self { Self::Auto }
}

impl std::fmt::Display for ExplorerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto"),
            Self::Manual => write!(f, "Manual"),
        }
    }
}

/// Planet statistics
#[derive(Debug, Clone)]
pub struct PlanetStats {
    pub energy_level: f32,
    pub charged_cells: usize,
    pub has_rocket: bool,
    pub planet_type: PlanetType,
    pub available_resources: Vec<String>,
}

impl Default for PlanetStats {
    fn default() -> Self {
        Self {
            energy_level: 0.0,
            charged_cells: 0,
            has_rocket: false,
            planet_type: PlanetType::Unknown,
            available_resources: Vec::new(),
        }
    }
}

/// Explorer statistics
#[derive(Debug, Clone)]
pub struct ExplorerStats {
    pub health: f32,
    pub bag_count: usize,
    pub ai_active: bool,
    pub mode: ExplorerMode,
}

impl Default for ExplorerStats {
    fn default() -> Self {
        Self {
            health: 1.0,
            bag_count: 0,
            ai_active: true,
            mode: ExplorerMode::default(),
        }
    }
}

/// Game-wide statistics
#[derive(Debug, Clone, Default)]
pub struct GameStats {
    pub asteroids_sent: u32,
    pub sunrays_sent: u32,
    pub planets_destroyed: u32,
    pub explorers_killed: u32,
    pub resources_generated: u32,
}

/// Tracks the complete state of the galaxy simulation.
#[derive(Debug, Clone)]
pub struct SystemState {
    galaxy_structure: GalaxyStructure,
    game_state: GameState,
    explorer_locations: HashMap<ID, ID>,
    planet_stats: HashMap<ID, PlanetStats>,
    explorer_stats: HashMap<ID, ExplorerStats>,
    game_stats: GameStats,
    explorers_ever_added: bool,
}

impl SystemState {
    /// Creates a new empty system state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            galaxy_structure: GalaxyStructure::new(),
            game_state: GameState::Running,
            explorer_locations: HashMap::new(),
            planet_stats: HashMap::new(),
            explorer_stats: HashMap::new(),
            game_stats: GameStats::default(),
            explorers_ever_added: false,
        }
    }

    /// Create a new system state from a file
    #[must_use]
    pub fn new_from_file(structure_file_path: String) -> Self {
        Self {
            galaxy_structure: Self::new_galaxy_from_file(structure_file_path),
            game_state: GameState::Running,
            explorer_locations: HashMap::new(),
            planet_stats: HashMap::new(),
            explorer_stats: HashMap::new(),
            game_stats: GameStats::default(),
            explorers_ever_added: false,
        }
    }

    #[must_use]
    pub fn new_galaxy_from_file(structure_file: String) -> GalaxyStructure {
        match Self::read_lines_to_vec(structure_file) {
            Ok(lines) => GalaxyStructure::new_from_file(lines.as_slice()),
            Err(_) => {
                log::warn!("Failed to read galaxy structure file, using empty galaxy");
                GalaxyStructure::new()
            }
        }
    }

    #[must_use]
    fn read_lines_to_vec<P>(filename: P) -> io::Result<Vec<String>>
    where P: AsRef<Path> {
        let file = fs::File::open(filename)?;
        let reader = io::BufReader::new(file);
        reader.lines().collect()
    }

    // ==================== Game State Management ====================

    #[must_use]
    pub fn game_state(&self) -> GameState { self.game_state }

    pub fn pause(&mut self) {
        if self.game_state == GameState::Running {
            self.game_state = GameState::Paused;
            log::info!("Game paused");
        }
    }

    pub fn resume(&mut self) {
        if self.game_state == GameState::Paused {
            self.game_state = GameState::Running;
            log::info!("Game resumed");
        }
    }

    pub fn end_game(&mut self) {
        self.game_state = GameState::Ended;
        log::info!("Game ended");
    }

    #[must_use]
    pub fn is_running(&self) -> bool { self.game_state == GameState::Running }
    #[must_use]
    pub fn is_paused(&self) -> bool { self.game_state == GameState::Paused }
    #[must_use]
    pub fn is_ended(&self) -> bool { self.game_state == GameState::Ended }
    #[must_use]
    pub fn should_continue(&self) -> bool { self.game_state != GameState::Ended }

    // ==================== Planet Management ====================

    pub fn add_planet(&mut self, planet_id: ID) {
        self.galaxy_structure.add_planet(planet_id, &[]);
        self.planet_stats.insert(planet_id, PlanetStats::default());
        log::info!("Planet {} added to galaxy", planet_id);
    }

    pub fn remove_planet(&mut self, planet_id: ID) {
        let explorers_to_remove: Vec<ID> = self.explorer_locations
            .iter()
            .filter_map(|(&eid, &loc)| if loc == planet_id { Some(eid) } else { None })
            .collect();

        for eid in explorers_to_remove {
            self.remove_explorer(eid);
        }

        self.galaxy_structure.remove_planet(planet_id);
        self.planet_stats.remove(&planet_id);
        self.game_stats.planets_destroyed += 1;

        log::info!("Planet {} removed from galaxy", planet_id);

        if self.galaxy_structure.get_alive_planets().is_empty() {
            log::warn!("All planets destroyed - ending game");
            self.end_game();
        }
    }

    #[must_use]
    pub fn is_planet_alive(&self, planet_id: ID) -> bool {
        self.galaxy_structure.is_alive(planet_id)
    }

    #[must_use]
    pub fn get_alive_planets_sorted(&self) -> Vec<ID> {
        let mut planets: Vec<ID> = self.galaxy_structure.get_alive_planets().iter().copied().collect();
        planets.sort_unstable();
        planets
    }

    #[must_use]
    pub fn get_adjacency(&self) -> &HashMap<ID, HashSet<ID>> {
        &self.galaxy_structure.get_adjacency()
    }

    #[must_use]
    pub fn get_adjacents(&self, planet_id: ID) -> HashSet<ID> {
        self.galaxy_structure.get_adjacents(planet_id)
    }

    pub fn update_planet_stats(&mut self, planet_id: ID, stats: PlanetStats) {
        self.planet_stats.insert(planet_id, stats);
    }

    #[must_use]
    pub fn get_planet_stats(&self, planet_id: ID) -> Option<&PlanetStats> {
        self.planet_stats.get(&planet_id)
    }

    // ==================== Explorer Management ====================

    pub fn add_explorer(&mut self, explorer_id: ID, planet_id: ID) -> Result<(), String> {
        if !self.galaxy_structure.is_alive(planet_id) {
            return Err(format!("Planet {planet_id} does not exist"));
        }
        self.explorer_locations.insert(explorer_id, planet_id);
        self.explorer_stats.insert(explorer_id, ExplorerStats::default());
        self.explorers_ever_added = true;
        log::info!("Explorer {} added on planet {}", explorer_id, planet_id);
        Ok(())
    }

    pub fn remove_explorer(&mut self, explorer_id: ID) {
        self.explorer_locations.remove(&explorer_id);
        self.explorer_stats.remove(&explorer_id);
        self.game_stats.explorers_killed += 1;
        log::info!("Explorer {} removed", explorer_id);

        if self.explorers_ever_added && self.explorer_locations.is_empty() {
            log::warn!("All explorers eliminated - ending game");
            self.end_game();
        }
    }

    #[must_use]
    pub fn explorer_location(&self, explorer_id: ID) -> Option<ID> {
        self.explorer_locations.get(&explorer_id).copied()
    }

    /// **Nuovo metodo pubblico** per restituire tutte le location degli explorer
    #[must_use]
    pub fn explorer_locations(&self) -> &HashMap<ID, ID> {
        &self.explorer_locations
    }

    #[must_use]
    pub fn get_explorers_on_planet(&self, planet_id: ID) -> Vec<ID> {
        let mut explorers: Vec<ID> = self.explorer_locations
            .iter()
            .filter_map(|(&eid, &loc)| if loc == planet_id { Some(eid) } else { None })
            .collect();
        explorers.sort_unstable();
        explorers
    }

    pub fn move_explorer(&mut self, explorer_id: ID, from_planet: ID, to_planet: ID) -> Result<(), String> {
        let current = self.explorer_locations.get(&explorer_id)
            .ok_or_else(|| format!("Explorer {explorer_id} does not exist"))?;

        if *current != from_planet {
            return Err(format!("Explorer {explorer_id} is not on planet {from_planet}"));
        }

        if !self.galaxy_structure.can_travel(from_planet, to_planet) {
            return Err(format!("Cannot travel from {from_planet} to {to_planet}"));
        }

        self.explorer_locations.insert(explorer_id, to_planet);
        Ok(())
    }

    pub fn update_explorer_stats(&mut self, explorer_id: ID, stats: ExplorerStats) {
        self.explorer_stats.insert(explorer_id, stats);
    }

    #[must_use]
    pub fn explorer_stats(&self, explorer_id: ID) -> Option<&ExplorerStats> {
        self.explorer_stats.get(&explorer_id)
    }

    // ==================== Adjacency Management ====================

    pub fn add_adjacency(&mut self, planet_a: ID, planet_b: ID) {
        if self.is_planet_alive(planet_a) && self.is_planet_alive(planet_b) {
            self.galaxy_structure.add_connection(planet_a, planet_b);
            log::info!("Added adjacency between {} and {}", planet_a, planet_b);
        } else {
            log::warn!("Cannot connect {} and {}: one or both planets dead", planet_a, planet_b);
        }
    }

    pub fn add_adjacency_unchecked(&mut self, planet_a: ID, planet_b: ID) {
        self.galaxy_structure.add_connection_unchecked(planet_a, planet_b);
        log::info!("Added unchecked adjacency between {} and {}", planet_a, planet_b);
    }

    #[must_use]
    pub fn can_travel(&self, planet_a: ID, planet_b: ID) -> bool {
        self.galaxy_structure.can_travel(planet_a, planet_b)
    }

    #[must_use]
    pub fn get_neighbors(&self, planet_id: ID) -> Vec<ID> {
        self.galaxy_structure.get_adjacency()
            .get(&planet_id)
            .map(|neighbors| {
                let mut sorted: Vec<ID> = neighbors.iter().copied().collect();
                sorted.sort_unstable();
                sorted
            })
            .unwrap_or_default()
    }

    // ==================== Statistics ====================

    pub fn increment_asteroids_sent(&mut self) { self.game_stats.asteroids_sent += 1; }
    pub fn increment_sunrays_sent(&mut self) { self.game_stats.sunrays_sent += 1; }
    pub fn increment_resources_generated(&mut self) { self.game_stats.resources_generated += 1; }

    #[must_use]
    pub fn game_stats(&self) -> &GameStats { &self.game_stats }
}

impl Default for SystemState {
    fn default() -> Self { Self::new() }
}

