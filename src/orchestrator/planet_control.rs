//! Planet message handling and control.
//!
//! This module handles incoming messages from planets and coordinates
//! planet-related operations like destruction, asteroid defense, and
//! resource management.

use common_game::protocols::orchestrator_planet::PlanetToOrchestrator;
use common_game::utils::ID;
use common_game::protocols::orchestrator_explorer::OrchestratorToExplorer;
use crate::orchestrator::Orchestrator;
use crate::orchestrator::gui_interface::GuiEvent;

/// Handles incoming messages from a planet.
pub(crate) fn handle_planet_msg(
    orchestrator: &mut Orchestrator,
    planet_id: ID,
    msg: PlanetToOrchestrator,
) {
    match msg {
        PlanetToOrchestrator::KillPlanetResult { planet_id: msg_planet_id } => {
            debug_assert_eq!(planet_id, msg_planet_id, "Planet ID mismatch");
            handle_planet_destruction(orchestrator, planet_id);
        }
        PlanetToOrchestrator::AsteroidAck { planet_id: msg_planet_id, rocket } => {
            debug_assert_eq!(planet_id, msg_planet_id, "Planet ID mismatch");
            handle_asteroid_ack(orchestrator, planet_id, rocket);
        }
        PlanetToOrchestrator::SunrayAck { planet_id: msg_planet_id } => {
            debug_assert_eq!(planet_id, msg_planet_id, "Planet ID mismatch");
            handle_sunray_ack(orchestrator, planet_id);
        }
        PlanetToOrchestrator::InternalStateResponse { planet_id: msg_planet_id, planet_state } => {
            debug_assert_eq!(planet_id, msg_planet_id, "Planet ID mismatch");
            handle_internal_state_response(orchestrator, planet_id, planet_state);
        }
        PlanetToOrchestrator::IncomingExplorerResponse { planet_id: msg_planet_id, explorer_id, res } => {
            debug_assert_eq!(planet_id, msg_planet_id, "Planet ID mismatch");
            handle_incoming_explorer_response(orchestrator, planet_id, explorer_id, res);
        }
        PlanetToOrchestrator::OutgoingExplorerResponse { planet_id: msg_planet_id, explorer_id, res } => {
            debug_assert_eq!(planet_id, msg_planet_id, "Planet ID mismatch");
            handle_outgoing_explorer_response(orchestrator, planet_id, explorer_id, res);
        }
        PlanetToOrchestrator::StartPlanetAIResult { planet_id: msg_planet_id }
        | PlanetToOrchestrator::StopPlanetAIResult { planet_id: msg_planet_id }
        | PlanetToOrchestrator::Stopped { planet_id: msg_planet_id } => {
            debug_assert_eq!(planet_id, msg_planet_id, "Planet ID mismatch");
            log::debug!("Planet {planet_id} acknowledged command");
        }
    }
}

/// Handles planet destruction and explorer cleanup.
fn handle_planet_destruction(orchestrator: &mut Orchestrator, planet_id: ID) {
    log::info!("Planet {planet_id} has been destroyed");

    // Remove all explorers on this planet
    let doomed_explorers = orchestrator.state.get_explorers_on_planet(planet_id);
    for explorer_id in doomed_explorers {
        if let Some(tx) = orchestrator.explorer_senders.get(&explorer_id) {
            let _ = tx.send(OrchestratorToExplorer::KillExplorer);
        }
        orchestrator.state.remove_explorer(explorer_id);
        let _ = orchestrator.gui_event_sender.send(
            crate::orchestrator::gui_interface::GuiEvent::ExplorerRemoved(explorer_id)
        );
        log::debug!("Explorer {} removed due to planet destruction", explorer_id);
    }

    orchestrator.get_galaxy_ai_mut().notify_planet_removed(planet_id);
    orchestrator.state.remove_planet(planet_id);
    orchestrator.planet_senders.remove(&planet_id);
    orchestrator.planet_receivers.remove(&planet_id);

    let _ = orchestrator.gui_event_sender.send(
        crate::orchestrator::gui_interface::GuiEvent::PlanetRemoved(planet_id)
    );
}

/// Handles asteroid acknowledgment from a planet.
fn handle_asteroid_ack(
    orchestrator: &mut Orchestrator,
    planet_id: ID,
    rocket: Option<common_game::components::rocket::Rocket>,
) {
    // Only forward this ack to the Galaxy AI if it's the shot the AI is
    // actually waiting on (as opposed to a GUI-triggered asteroid).
    let was_ai_pending = orchestrator.get_galaxy_ai().get_pending_target() == Some(planet_id);
    if was_ai_pending {
        orchestrator
            .get_galaxy_ai_mut()
            .notify_asteroid_result(planet_id, rocket.is_none());

        let intervals = orchestrator.get_galaxy_ai().get_cooldown_intervals_remaining();
        let _ = orchestrator
            .gui_event_sender
            .send(GuiEvent::GalaxyAiCooldownStarted { intervals });
    }

    match rocket {
        Some(_) => {
            log::info!("Planet {planet_id} successfully defended against asteroid");
            let _ = orchestrator.gui_event_sender.send(
                crate::orchestrator::gui_interface::GuiEvent::AsteroidHit(planet_id, true)
            );
        }
        None => {
            log::warn!("Planet {planet_id} couldn't defend against asteroid, killing it");
            if let Some(tx) = orchestrator.planet_senders.get(&planet_id) {
                let _ = tx.send(common_game::protocols::orchestrator_planet::OrchestratorToPlanet::KillPlanet);
            }
            let _ = orchestrator.gui_event_sender.send(
                crate::orchestrator::gui_interface::GuiEvent::AsteroidHit(planet_id, false)
            );
        }
    }
}

/// Handles sunray acknowledgment from a planet.
fn handle_sunray_ack(orchestrator: &mut Orchestrator, planet_id: ID) {
    log::debug!("Planet {planet_id} acknowledged sunray");
    let _ = orchestrator.gui_event_sender.send(
        crate::orchestrator::gui_interface::GuiEvent::SunrayReceived(planet_id)
    );
}

/// Handles internal state response from a planet.
fn handle_internal_state_response(
    orchestrator: &mut Orchestrator,
    planet_id: ID,
    state: common_game::components::planet::DummyPlanetState,
) {
    let energy_level = if state.energy_cells.is_empty() {
        0.0
    } else {
        state.charged_cells_count as f32 / state.energy_cells.len() as f32
    };

    let stats = crate::orchestrator::state::PlanetStats {
        energy_level,
        charged_cells: state.charged_cells_count,
        has_rocket: state.has_rocket,
        planet_type: crate::orchestrator::state::PlanetType::Unknown,
        available_resources: Vec::new(),
    };

    orchestrator.state.update_planet_stats(planet_id, stats);

    let _ = orchestrator.gui_event_sender.send(
        crate::orchestrator::gui_interface::GuiEvent::PlanetStateUpdated(planet_id)
    );
}

/// Handles incoming explorer response from a planet.
fn handle_incoming_explorer_response(
    orchestrator: &mut Orchestrator,
    planet_id: ID,
    explorer_id: ID,
    res: Result<(), String>,
) {
    match res {
        Ok(()) => {
            log::info!("Explorer {explorer_id} successfully arrived at planet {planet_id}");
            let _ = orchestrator.gui_event_sender.send(
                crate::orchestrator::gui_interface::GuiEvent::ExplorerArrived(explorer_id, planet_id)
            );
        }
        Err(e) => {
            log::error!("Explorer {explorer_id} rejected by planet {planet_id}: {e}");
            if let Some(tx) = orchestrator.explorer_senders.get(&explorer_id) {
                let _ = tx.send(OrchestratorToExplorer::MoveToPlanet {
                    planet_id,
                    sender_to_new_planet: None,
                });
            }
            let _ = orchestrator.gui_event_sender.send(
                crate::orchestrator::gui_interface::GuiEvent::ExplorerMoveRejected(explorer_id, planet_id, e)
            );
        }
    }
}

/// Handles outgoing explorer response from a planet.
fn handle_outgoing_explorer_response(
    _orchestrator: &mut Orchestrator,
    planet_id: ID,
    explorer_id: ID,
    res: Result<(), String>,
) {
    match res {
        Ok(()) => log::debug!("Explorer {explorer_id} departed planet {planet_id} successfully"),
        Err(e) => log::warn!("Planet {planet_id} failed explorer {explorer_id} departure: {e}"),
    }
}