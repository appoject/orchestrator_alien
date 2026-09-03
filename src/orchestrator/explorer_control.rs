//! Explorer message handling and control.
//!
//! This module handles incoming messages from explorers and coordinates
//! explorer-related operations like movement, resource requests, and
//! AI control.

use common_game::components::resource::GenericResource;
use common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator;
use common_game::utils::ID;

use crate::orchestrator::Orchestrator;
use super::routing;

/// Handles incoming messages from an explorer.
pub(crate) fn handle_explorer_msg(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    msg: ExplorerToOrchestrator<GenericResource>,
) {
    match msg {
        ExplorerToOrchestrator::TravelToPlanetRequest {
            explorer_id: msg_explorer_id,
            current_planet_id,
            dst_planet_id,
        } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            routing::handle_travel_request(
                orchestrator,
                explorer_id,
                current_planet_id,
                dst_planet_id,
            );
        }
        ExplorerToOrchestrator::NeighborsRequest {
            explorer_id: msg_explorer_id,
            current_planet_id,
        } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            handle_neighbors_request(orchestrator, explorer_id, current_planet_id);
        }
        ExplorerToOrchestrator::BagContentResponse {
            explorer_id: msg_explorer_id,
            bag_content,
        } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            handle_bag_content_response(orchestrator, explorer_id, bag_content);
        }
        ExplorerToOrchestrator::SupportedResourceResult {
            explorer_id: msg_explorer_id,
            supported_resources,
        } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            handle_supported_resource_result(orchestrator, explorer_id, supported_resources);
        }
        ExplorerToOrchestrator::SupportedCombinationResult {
            explorer_id: msg_explorer_id,
            combination_list,
        } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            handle_supported_combination_result(orchestrator, explorer_id, combination_list);
        }
        ExplorerToOrchestrator::GenerateResourceResponse {
            explorer_id: msg_explorer_id,
            generated,
        } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            handle_generate_resource_response(orchestrator, explorer_id, generated);
        }
        ExplorerToOrchestrator::CombineResourceResponse {
            explorer_id: msg_explorer_id,
            generated,
        } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            handle_combine_resource_response(orchestrator, explorer_id, generated);
        }
        ExplorerToOrchestrator::CurrentPlanetResult {
            explorer_id: msg_explorer_id,
            planet_id,
        } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            handle_current_planet_result(orchestrator, explorer_id, planet_id);
        }
        ExplorerToOrchestrator::StartExplorerAIResult { explorer_id: msg_explorer_id }
        | ExplorerToOrchestrator::KillExplorerResult { explorer_id: msg_explorer_id }
        | ExplorerToOrchestrator::ResetExplorerAIResult { explorer_id: msg_explorer_id }
        | ExplorerToOrchestrator::StopExplorerAIResult { explorer_id: msg_explorer_id }
        | ExplorerToOrchestrator::MovedToPlanetResult { explorer_id: msg_explorer_id, .. } => {
            debug_assert_eq!(
                explorer_id, msg_explorer_id,
                "Explorer ID mismatch: param={explorer_id}, msg={msg_explorer_id}"
            );
            log::debug!("Explorer {msg_explorer_id} acknowledged command");
        }
    }
}

/// Handles a neighbors request from an explorer.
fn handle_neighbors_request(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    current_planet_id: ID,
) {
    let neighbors = orchestrator.state.get_neighbors(current_planet_id);

    if let Some(tx) = orchestrator.explorer_senders.get(&explorer_id) {
        if tx.send(common_game::protocols::orchestrator_explorer::OrchestratorToExplorer::NeighborsResponse {
            neighbors,
        }).is_err() {
            log::warn!("Failed to send neighbors to explorer {explorer_id} (disconnected)");
        }
        log::debug!("Sent neighbors to explorer {explorer_id}");
    }
}

/// Handles bag content response from an explorer.
fn handle_bag_content_response(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    _bag_content: GenericResource,
) {
    // TODO: Compute actual bag count from bag_content
    let estimated_bag_count = 1;

    if let Some(stats) = orchestrator.state.explorer_stats(explorer_id) {
        let mut updated_stats = stats.clone();
        updated_stats.bag_count = estimated_bag_count;

        orchestrator.state.update_explorer_stats(explorer_id, updated_stats);
    }

    let _ = orchestrator.gui_event_sender.send(
        crate::orchestrator::gui_interface::GuiEvent::ExplorerBagUpdated(explorer_id)
    );
}

/// Handles supported resource result from an explorer.
fn handle_supported_resource_result(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    supported_resources: std::collections::HashSet<common_game::components::resource::BasicResourceType>,
) {
    log::debug!("Explorer {explorer_id} reported supported resources: {:?}", supported_resources);

    let _ = orchestrator.gui_event_sender.send(
        crate::orchestrator::gui_interface::GuiEvent::ResourcesDiscovered(explorer_id)
    );
}

/// Handles supported combination result from an explorer.
fn handle_supported_combination_result(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    combination_list: std::collections::HashSet<common_game::components::resource::ComplexResourceType>,
) {
    log::debug!("Explorer {explorer_id} reported supported combinations: {:?}", combination_list);

    let _ = orchestrator.gui_event_sender.send(
        crate::orchestrator::gui_interface::GuiEvent::CombinationsDiscovered(explorer_id)
    );
}

/// Handles generate resource response from an explorer.
fn handle_generate_resource_response(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    generated: Result<(), String>,
) {
    match generated {
        Ok(()) => {
            log::info!("Explorer {explorer_id} successfully generated a resource");

            let _ = orchestrator.gui_event_sender.send(
                crate::orchestrator::gui_interface::GuiEvent::ResourceGenerated(explorer_id, true)
            );
        }
        Err(e) => {
            log::warn!("Explorer {explorer_id} failed to generate resource: {e}");

            let _ = orchestrator.gui_event_sender.send(
                crate::orchestrator::gui_interface::GuiEvent::ResourceGenerated(explorer_id, false)
            );
        }
    }
}

/// Handles combine resource response from an explorer.
fn handle_combine_resource_response(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    generated: Result<(), String>,
) {
    match generated {
        Ok(()) => {
            log::info!("Explorer {explorer_id} successfully combined resources");

            let _ = orchestrator.gui_event_sender.send(
                crate::orchestrator::gui_interface::GuiEvent::ResourceCombined(explorer_id, true)
            );
        }
        Err(e) => {
            log::warn!("Explorer {explorer_id} failed to combine resources: {e}");

            let _ = orchestrator.gui_event_sender.send(
                crate::orchestrator::gui_interface::GuiEvent::ResourceCombined(explorer_id, false)
            );
        }
    }
}

/// Handles current planet result from an explorer.
fn handle_current_planet_result(
    orchestrator: &mut Orchestrator,
    explorer_id: ID,
    planet_id: ID,
) {
    log::debug!("Explorer {explorer_id} is on planet {planet_id}");

    if let Some(current_location) = orchestrator.state.explorer_location(explorer_id) {
        if current_location != planet_id {
            log::warn!("Explorer {explorer_id} location mismatch: recorded {current_location}, reported {planet_id}");
        }
    }

    let _ = orchestrator.gui_event_sender.send(
        crate::orchestrator::gui_interface::GuiEvent::ExplorerLocationConfirmed(explorer_id, planet_id)
    );
}
