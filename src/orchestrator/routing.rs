//! Message routing and travel logic.
//!
//! This module handles routing of messages between actors and coordinates
//! travel requests between planets.

use crossbeam_channel::unbounded;
use common_game::protocols::orchestrator_explorer::OrchestratorToExplorer;
use common_game::protocols::orchestrator_planet::OrchestratorToPlanet;
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use common_game::utils::ID;

use crate::orchestrator::Orchestrator;
use crate::orchestrator::gui_interface::GuiEvent;

/// Handles a travel request from an explorer.
pub(crate) fn handle_travel_request(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    from_planet_id: ID,
    to_planet_id: ID,
) {
    log::info!(
        "Handling travel request: explorer {explorer_id} from {from_planet_id} to {to_planet_id}"
    );

    // Validate adjacency and alive planets
    if !orchestrator.state.can_travel(from_planet_id, to_planet_id) {
        log::warn!("Travel request rejected: planets not adjacent or some are dead");
        send_rejection(
            orchestrator,
            explorer_id,
            from_planet_id,
            to_planet_id,
            "Planets not adjacent or dead",
        );
        return;
    }

    // Create bidirectional communication channels
    let (planet_to_explorer_sender, _planet_to_explorer_receiver) = unbounded::<PlanetToExplorer>();
    let (explorer_to_planet_sender, _explorer_to_planet_receiver) = unbounded::<ExplorerToPlanet>();

    // Notify destination planet
    if let Some(dest_tx) = orchestrator.planet_senders.get(&to_planet_id) {
        if dest_tx.send(OrchestratorToPlanet::IncomingExplorerRequest {
            explorer_id,
            new_sender: planet_to_explorer_sender,
        }).is_err() {
            log::error!("Failed to notify destination planet {to_planet_id}");
            send_rejection(
                orchestrator,
                explorer_id,
                from_planet_id,
                to_planet_id,
                "Destination planet unreachable",
            );
            return;
        }
    } else {
        log::error!("Destination planet not found: {to_planet_id}");
        send_rejection(
            orchestrator,
            explorer_id,
            from_planet_id,
            to_planet_id,
            "Destination planet not found",
        );
        return;
    }

    // Notify source planet
    if let Some(source_tx) = orchestrator.planet_senders.get(&from_planet_id) {
        let _ = source_tx.send(OrchestratorToPlanet::OutgoingExplorerRequest { explorer_id });
    }

    // Update system state
    if let Err(e) = orchestrator.state.move_explorer(explorer_id, from_planet_id, to_planet_id) {
        log::error!("Failed to update system state: {e}");
        send_rejection(
            orchestrator,
            explorer_id,
            from_planet_id,
            to_planet_id,
            &format!("State update failed: {e}"),
        );
        return;
    }

    // Notify explorer
    if let Some(tx) = orchestrator.explorer_senders.get(&explorer_id) {
        if tx.send(OrchestratorToExplorer::MoveToPlanet {
            sender_to_new_planet: Some(explorer_to_planet_sender),
            planet_id: to_planet_id,
        }).is_err() {
            log::error!("Failed to notify explorer {explorer_id}");
            // Rollback state
            let _ = orchestrator.state.move_explorer(explorer_id, to_planet_id, from_planet_id);

            let _ = orchestrator.gui_event_sender.send(GuiEvent::ExplorerMoveRejected(
                explorer_id,
                to_planet_id,
                "Failed to send channel".to_string(),
            ));
        } else {
            log::info!("Explorer {explorer_id} successfully moved to planet {to_planet_id}");
            let _ = orchestrator.gui_event_sender.send(GuiEvent::ExplorerMoved(
                explorer_id,
                from_planet_id,
                to_planet_id,
            ));

        }
    } else {
        log::error!("Explorer sender not found for {explorer_id}");
        let _ = orchestrator.state.move_explorer(explorer_id, to_planet_id, from_planet_id);
        let _ = orchestrator.gui_event_sender.send(GuiEvent::ExplorerMoveRejected(
            explorer_id,
            to_planet_id,
            "Explorer sender not found".to_string(),
        ));
    }
}

/// Helper function to send rejection notifications to both explorer and GUI
fn send_rejection(
    orchestrator: &Orchestrator,
    explorer_id: ID,
    from_planet_id: ID,
    to_planet_id: ID,
    reason: &str,
) {
    if let Some(tx) = orchestrator.explorer_senders.get(&explorer_id) {
        let _ = tx.send(OrchestratorToExplorer::MoveToPlanet {
            sender_to_new_planet: None,
            planet_id: from_planet_id,
        });
    }

    let _ = orchestrator.gui_event_sender.send(GuiEvent::ExplorerMoveRejected(
        explorer_id,
        to_planet_id,
        reason.to_string(),
    ));
}

