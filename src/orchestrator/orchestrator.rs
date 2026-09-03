//! Main orchestrator implementation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded};

use common_game::components::forge::Forge;
use common_game::components::resource::GenericResource;
use common_game::protocols::orchestrator_explorer::{
    ExplorerToOrchestrator, OrchestratorToExplorer,
};
use common_game::protocols::orchestrator_planet::{
    OrchestratorToPlanet, PlanetToOrchestrator,
};
use common_game::utils::ID;

use crate::orchestrator::state::SystemState;
use crate::orchestrator::gui_interface::{GuiEvent, GuiState, GuiCommand};
use crate::orchestrator::galaxy_ai::{GalaxyAI, GalaxyAction};

// Import internal handlers from parent module
use super::planet_control;
use super::explorer_control;

//use crate::orchestrator::{planet_control, explorer_control};

/// Main orchestrator structure that manages the entire simulation.
pub struct Orchestrator {
    forge: Forge,

    /// Planet communication channels
    pub planet_senders: HashMap<ID, Sender<OrchestratorToPlanet>>,
    pub(crate) planet_receivers: HashMap<ID, Receiver<PlanetToOrchestrator>>,

    /// Explorer communication channels
    pub explorer_senders: HashMap<ID, Sender<OrchestratorToExplorer>>,
    explorer_receivers: HashMap<ID, Receiver<ExplorerToOrchestrator<GenericResource>>>,

    /// System state
    pub state: SystemState,

    /// GUI channels
    pub(crate) gui_event_sender: Sender<GuiEvent>,
    gui_event_receiver: Receiver<GuiEvent>,
    pub(crate) gui_command_sender: Sender<GuiCommand>,
    gui_command_receiver: Receiver<GuiCommand>,

    /// Galaxy AI
    galaxy_ai: GalaxyAI,

    /// Stop flag
    pub(crate) should_stop: Arc<AtomicBool>,

    /// Cycle duration
    pub(crate) cycle_duration_in_millis: u64,
}

impl Orchestrator {
    /// Create a new orchestrator (empty)
    pub fn new() -> Result<Self, String> {
        let forge = Forge::new().map_err(|e| format!("Failed to create forge: {e}"))?;
        let (gui_event_sender, gui_event_receiver) = bounded(1000);
        let (gui_command_sender, gui_command_receiver) = bounded(100);

        Ok(Self {
            forge,
            planet_senders: HashMap::new(),
            planet_receivers: HashMap::new(),
            explorer_senders: HashMap::new(),
            explorer_receivers: HashMap::new(),
            state: SystemState::new(),
            gui_event_sender,
            gui_event_receiver,
            gui_command_sender,
            gui_command_receiver,
            galaxy_ai: GalaxyAI::new_inactive(),
            should_stop: Arc::new(AtomicBool::new(false)),
            cycle_duration_in_millis: 500,
        })
    }

    /// Create orchestrator with pre-defined planets/explorers and galaxy structure
    pub fn new_with_parameters(
        planet_senders: HashMap<ID, Sender<OrchestratorToPlanet>>,
        planet_receivers: HashMap<ID, Receiver<PlanetToOrchestrator>>,
        explorer_senders: HashMap<ID, Sender<OrchestratorToExplorer>>,
        explorer_receivers: HashMap<ID, Receiver<ExplorerToOrchestrator<GenericResource>>>,
        galaxy_structure_file: String,
    ) -> Result<Self, String> {
        let forge = Forge::new().map_err(|e| format!("Failed to create forge: {e}"))?;
        let (gui_event_sender, gui_event_receiver) = bounded(1000);
        let (gui_command_sender, gui_command_receiver) = bounded(100);

        Ok(Self {
            forge,
            planet_senders,
            planet_receivers,
            explorer_senders,
            explorer_receivers,
            state: SystemState::new_from_file(galaxy_structure_file),
            gui_event_sender,
            gui_event_receiver,
            gui_command_sender,
            gui_command_receiver,
            galaxy_ai: GalaxyAI::new_active(),
            should_stop: Arc::new(AtomicBool::new(false)),
            cycle_duration_in_millis: 500,
        })
    }

    // ==================== Entity Management ====================

    pub fn add_planet(
        &mut self,
        planet_id: ID,
        sender: Sender<OrchestratorToPlanet>,
        receiver: Receiver<PlanetToOrchestrator>,
    ) -> Result<(), String> {
        if self.planet_senders.contains_key(&planet_id) {
            return Err(format!("Planet with ID {planet_id} already exists"));
        }

        self.planet_senders.insert(planet_id, sender);
        self.planet_receivers.insert(planet_id, receiver);
        self.state.add_planet(planet_id);

        let _ = self.gui_event_sender.send(GuiEvent::PlanetAdded(planet_id));
        Ok(())
    }

    pub fn add_explorer(
        &mut self,
        explorer_id: ID,
        to_explorer_tx: Sender<OrchestratorToExplorer>,
        from_explorer_rx: Receiver<ExplorerToOrchestrator<GenericResource>>,
        planet_id: ID,
    ) -> Result<(), String> {
        if !self.state.is_planet_alive(planet_id) {
            return Err(format!("Planet {planet_id} is dead"));
        }

        self.explorer_senders.insert(explorer_id, to_explorer_tx);
        self.explorer_receivers.insert(explorer_id, from_explorer_rx);
        self.state.add_explorer(explorer_id, planet_id)?;

        let _ = self.gui_event_sender.send(GuiEvent::ExplorerAdded(explorer_id, planet_id));
        Ok(())
    }

    // ==================== Galaxy AI ====================

    pub fn enable_galaxy_ai(&mut self) {
        log::info!("Galaxy AI enabled");
        self.galaxy_ai.enable_ai();
    }

    pub fn disable_galaxy_ai(&mut self) {
        log::info!("Galaxy AI disabled");
        self.galaxy_ai.disable_ai();
    }

    #[must_use]
    pub fn is_galaxy_ai_enabled(&self) -> bool {
        self.galaxy_ai.is_enabled()
    }

    #[must_use]
    pub fn get_galaxy_ai(&self) -> &GalaxyAI {
        &self.galaxy_ai
    }

    #[must_use]
    pub fn get_galaxy_ai_mut(&mut self) -> &mut GalaxyAI {
        &mut self.galaxy_ai
    }

    // ==================== Main Loop ====================

    pub fn run(&mut self) -> Result<(), String> {
        log::info!("Orchestrator starting main loop");

        let mut last_gui_update = Instant::now();
        let gui_update_interval = Duration::from_millis(500);
        self.enable_galaxy_ai();

        while !self.should_stop.load(Ordering::Relaxed) && self.state.should_continue() {
            if self.state.is_running() {
                self.poll_all_messages();
                self.periodic_tasks();
                self.run_galaxy_ai();
            }

            let now = Instant::now();
            if now.duration_since(last_gui_update) >= gui_update_interval {
                self.update_gui_state();
                last_gui_update = now;
            }

            std::thread::sleep(Duration::from_millis(self.cycle_duration_in_millis));
        }

        log::info!("Orchestrator main loop stopped (game_state: {:?})", self.state.game_state());
        Ok(())
    }

    fn poll_all_messages(&mut self) {
        // Planet messages
        let mut planet_msgs = Vec::new();
        for (&planet_id, rx) in &self.planet_receivers {
            while let Ok(msg) = rx.try_recv() {
                planet_msgs.push((planet_id, msg));
            }
        }
        for (planet_id, msg) in planet_msgs {
            planet_control::handle_planet_msg(self, planet_id, msg);
        }

        // Explorer messages
        let mut explorer_msgs = Vec::new();
        for (&explorer_id, rx) in &self.explorer_receivers {
            while let Ok(msg) = rx.try_recv() {
                explorer_msgs.push((explorer_id, msg));
            }
        }
        for (explorer_id, msg) in explorer_msgs {
            explorer_control::handle_explorer_msg(self, explorer_id, msg);
        }

        // GUI commands
        while let Ok(command) = self.gui_command_receiver.try_recv() {
            self.handle_gui_command(command);
        }
    }

    fn periodic_tasks(&mut self) {
        let dead_planets: Vec<ID> = self.planet_receivers
            .iter()
            .filter_map(|(&id, rx)| {
                match rx.try_recv() {
                    Err(TryRecvError::Disconnected) => Some(id),
                    _ => None,
                }
            })
            .collect();

        for planet_id in dead_planets {
            self.destroy_planet(planet_id);
        }
    }

    fn run_galaxy_ai(&mut self) {
        let alive_planets = self.state.get_alive_planets_sorted();
        let targets_before = self.galaxy_ai.get_target_set().to_vec();

        let actions = self.galaxy_ai.update(&alive_planets);

        let targets_after = self.galaxy_ai.get_target_set().to_vec();
        if targets_after != targets_before && !targets_after.is_empty() {
            let _ = self.gui_event_sender.send(GuiEvent::GalaxyAiTargetsSelected(targets_after));
        }

        for action in actions {
            match action {
                GalaxyAction::SendAsteroid { target_planet } => {
                    log::info!("Galaxy AI decided to send asteroid to planet {target_planet}");
                    let _ = self.gui_event_sender.send(GuiEvent::GalaxyAiAsteroidLaunched(target_planet));
                    let _ = self.send_asteroid_to_planet(target_planet);
                }
                GalaxyAction::SendSunray { target_planet } => {
                    log::info!("Galaxy AI decided to send sunray to planet {target_planet}");
                    self.send_sunray_to_planet(target_planet);
                }
            }
        }
    }

    fn update_gui_state(&self) {
        let _ = self.gui_event_sender.send(GuiEvent::StateUpdate(self.get_gui_state()));
    }

    pub fn stop(&mut self) {
        log::info!("Stopping orchestrator...");
        self.should_stop.store(true, Ordering::Relaxed);
        self.disable_galaxy_ai();

        for (planet_id, sender) in &self.planet_senders {
            let _ = sender.send(OrchestratorToPlanet::KillPlanet);
            log::debug!("Sent KillPlanet to planet {planet_id}");
        }

        for (explorer_id, sender) in &self.explorer_senders {
            let _ = sender.send(OrchestratorToExplorer::KillExplorer);
            log::debug!("Sent KillExplorer to explorer {explorer_id}");
        }
    }

    // ==================== GUI Interface ====================

    #[must_use]
    pub fn get_gui_state(&self) -> GuiState {
        GuiState::from_system_state(&self.state, &self.galaxy_ai)
    }

    #[must_use]
    pub fn gui_event_receiver(&self) -> Receiver<GuiEvent> {
        self.gui_event_receiver.clone()
    }

    #[must_use]
    pub fn gui_command_sender(&self) -> Sender<GuiCommand> {
        self.gui_command_sender.clone()
    }

    fn handle_gui_command(&mut self, command: GuiCommand) {
        match command {
            GuiCommand::SendAsteroid { planet_id } => {
                let _ = self.send_asteroid_to_planet(planet_id);
            }
            GuiCommand::SendSunray { planet_id } => {
                self.send_sunray_to_planet(planet_id);
            }
            GuiCommand::TogglePlanetAI { planet_id, enabled } => {
                self.toggle_planet_ai(planet_id, enabled);
            }
            GuiCommand::ToggleExplorerAI { explorer_id, enabled } => {
                self.toggle_explorer_ai(explorer_id, enabled);
            }
            GuiCommand::PauseSimulation => {
                self.state.pause();
                log::info!("Simulation paused");
            }
            GuiCommand::ResumeSimulation => {
                self.state.resume();
                log::info!("Simulation resumed");
            }
            GuiCommand::SetSimulationCycleLengthInMillis { millis } => {
                self.cycle_duration_in_millis = millis;
                log::info!("Simulation cycle length in millis set to {millis}");
            }
            GuiCommand::EnableGalaxyAI => {
                self.enable_galaxy_ai();
            }
            GuiCommand::DisableGalaxyAI => {
                self.disable_galaxy_ai();
            }
        }
    }

    // ==================== Game Actions ====================

    pub fn send_asteroid_to_planet(&mut self, planet_id: ID) -> Result<(), String> {
        if !self.state.is_planet_alive(planet_id) {
            return Err("Planet is dead".into());
        }

        let tx = self.planet_senders.get(&planet_id).ok_or("Planet sender missing")?;
        let asteroid = self.forge.generate_asteroid();
        tx.send(OrchestratorToPlanet::Asteroid(asteroid)).map_err(|_| "Send failed")?;

        self.state.increment_asteroids_sent();
        let _ = self.gui_event_sender.send(GuiEvent::AsteroidSent(planet_id));
        Ok(())
    }

    pub(crate) fn send_sunray_to_planet(&mut self, planet_id: ID) {
        if let Some(sender) = self.planet_senders.get(&planet_id) {
            let sunray = self.forge.generate_sunray();
            if sender.send(OrchestratorToPlanet::Sunray(sunray)).is_ok() {
                self.state.increment_sunrays_sent();
                let _ = self.gui_event_sender.send(GuiEvent::SunraySent(planet_id));
            }
        }
    }

    pub(crate) fn toggle_planet_ai(&mut self, planet_id: ID, enabled: bool) {
        if let Some(sender) = self.planet_senders.get(&planet_id) {
            let msg = if enabled { OrchestratorToPlanet::StartPlanetAI } else { OrchestratorToPlanet::StopPlanetAI };
            let _ = sender.send(msg);
        }
    }

    fn toggle_explorer_ai(&mut self, explorer_id: ID, enabled: bool) {
        if let Some(sender) = self.explorer_senders.get(&explorer_id) {
            let msg = if enabled { OrchestratorToExplorer::StartExplorerAI } else { OrchestratorToExplorer::StopExplorerAI };
            let _ = sender.send(msg);
        }
    }

    // ==================== Planet/Explorer Lifecycle ====================

    pub(crate) fn destroy_planet(&mut self, planet_id: ID) {
        let explorers_on_planet: Vec<ID> = self.state.get_explorers_on_planet(planet_id);
        for explorer_id in explorers_on_planet {
            self.kill_explorer(explorer_id);
        }

        if let Some(sender) = self.planet_senders.get(&planet_id) {
            let _ = sender.send(OrchestratorToPlanet::KillPlanet);
        }

        self.planet_senders.remove(&planet_id);
        self.planet_receivers.remove(&planet_id);
        self.state.remove_planet(planet_id);
        let _ = self.gui_event_sender.send(GuiEvent::PlanetRemoved(planet_id));
    }

    pub(crate) fn kill_explorer(&mut self, explorer_id: ID) {
        if let Some(sender) = self.explorer_senders.get(&explorer_id) {
            let _ = sender.send(OrchestratorToExplorer::KillExplorer);
        }

        self.explorer_senders.remove(&explorer_id);
        self.explorer_receivers.remove(&explorer_id);
        self.state.remove_explorer(explorer_id);
        let _ = self.gui_event_sender.send(GuiEvent::ExplorerRemoved(explorer_id));
    }

    // ==================== Accessors ====================

    #[must_use]
    pub fn get_forge(&self) -> &Forge { &self.forge }

    #[must_use]
    pub fn get_state(&self) -> &SystemState { &self.state }
}

impl Default for Orchestrator {
    fn default() -> Self { Self::new().expect("Failed to create default Orchestrator") }
}