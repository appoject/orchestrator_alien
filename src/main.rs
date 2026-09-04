//! 🌌 AP Rover Project - Full Galaxy Simulation
//!
//! This runs a complete simulation with:
//! - 7 planets from different repositories
//! - 2 explorers
//! - Active Galaxy AI
//! - Real-time monitoring


use orchestrator_alien::logging;
use orchestrator_alien::Orchestrator;
use orchestrator_alien::SystemState;
use orchestrator_alien::GuiState;
use log::LevelFilter;
use crossbeam_channel::{unbounded, bounded};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;
use std::io::{self, Write};
use std::process::exit;
use houston::houston_we_have_a_borrow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("AP Rover Project - FULL GALAXY SIMULATION");
    println!("==============================================");
    println!("Starting with:");
    println!("• 7 Planets from different repositories");
    println!("• 2 Explorers");
    println!("• Active Galaxy AI");
    println!("• Real-time monitoring");
    println!("==============================================");

    // Initialize logging
    logging::init(LevelFilter::Info);

    log::info!("Initializing full galaxy simulation...");

    // Create orchestrator
    let mut orchestrator = Orchestrator::new()?;

    // Load galaxy structure
    let galaxy = load_galaxy_structure()?;
    println!("galaxy structure loaded: {} planets", galaxy.len());

    // Spawn all planets (each from a different repository)
    spawn_all_planets(&mut orchestrator, &galaxy)?;
    println!("all 7 planets spawned and initialized");

    // Spawn explorers (2 explorers as specified)
    spawn_explorers(&mut orchestrator, &galaxy)?;
    println!("2 explorers deployed");

    // Enable Galaxy AI
    orchestrator.enable_galaxy_ai();
    println!("galaxy AI enabled");

    // Create monitoring thread
    let monitoring_tx = create_monitoring_thread(&orchestrator);

    // Start orchestrator in background thread
    let orchestrator_handle = start_orchestrator_thread(orchestrator)?;

    // Run interactive console
    interactive_console(monitoring_tx)?;

    // Shutdown
    println!("\n exiting galaxy_simulation");
    orchestrator_handle.join()
        .map_err(|_| "failed to join orchestrator thread")?
        .map_err(|e| format!("orchestrator error: {}", e))?;

    println!("simulation completed successfully!");
    Ok(())
}

// ==================== GALAXY CONFIGURATION ====================

fn load_galaxy_structure() -> Result<Vec<(u32, Vec<u32>)>, Box<dyn std::error::Error>> {
    // Read from galaxy.txt or use default structure
    match std::fs::read_to_string("galaxy.txt") {
        Ok(content) => {
            let mut galaxy = Vec::new();
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                let planet_id: u32 = parts[0].parse()?;
                let neighbors: Vec<u32> = parts[1..]
                    .iter()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                galaxy.push((planet_id, neighbors));
            }
            Ok(galaxy)
        }
        Err(_) => {
            // Default galaxy structure
            println!("no galaxy.txt found, using default hexagonal galaxy");
            Ok(vec![
                (1, vec![2, 3, 4,5,6,7]),
                (2, vec![1,3,4,5,6,7]),
                (3, vec![1,2,4,5,6,7]),
                (4, vec![1,2,3,5,6,7]),
                (5, vec![1,2,3,4,6,7]),
                (6, vec![1,2,3,4,5,7]),
                (7, vec![1, 2, 3, 4, 5, 6]),
            ])
        }
    }
}

// ==================== PLANET SPAWNING ====================

fn spawn_all_planets(
    orchestrator: &mut Orchestrator,
    galaxy: &[(u32, Vec<u32>)],
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("spawning {} planets...", galaxy.len());

    // Planet repository mapping
    let planet_repos = [
        ("orbitron", "Advanced-Programming-2025-Orbitron/Orbitron"),
        ("skycartel", "0TH08/Skycartel"),
        ("enterprise", "Thompspsps/Enterprise_planet"),
        ("rustrelli", "Rustrelli/rustrelli"),
        ("the_compiler_strikes_back", "TheCompilerStrikesBackAP2025/TheCompilerStrikesBack"),
        ("crabtorio", "crabtorio/crabtorio"),
        ("houston", "Houston-we-have-a-borrow/Planet"),

    ];

    for (i, &(planet_id, ref neighbors)) in galaxy.iter().enumerate() {
        let repo_name = planet_repos[i % planet_repos.len()].0;

        // Create communication channels
        let (to_planet_tx, to_planet_rx) = unbounded();
        let (from_planet_tx, from_planet_rx) = unbounded();

        // Spawn planet thread (using appropriate crate or fallback)
        spawn_planet_thread(
            planet_id,
            repo_name,
            neighbors.clone(),
            to_planet_rx,
            from_planet_tx,
        )?;

        // Add to orchestrator
        orchestrator.add_planet(planet_id, to_planet_tx, from_planet_rx)?;

        // Add adjacency to state
        for &neighbor in neighbors {
            orchestrator.state.add_adjacency_unchecked(planet_id, neighbor);
        }

        println!("  planet {} - {} (neighbors: {:?})", planet_id, repo_name, neighbors);
    }

    Ok(())
}

fn spawn_planet_thread(
    planet_id: u32,
    repo_name: &str,
    neighbors: Vec<u32>,
    rx_orchestrator: crossbeam_channel::Receiver<common_game::protocols::orchestrator_planet::OrchestratorToPlanet>,
    tx_orchestrator: crossbeam_channel::Sender<common_game::protocols::orchestrator_planet::PlanetToOrchestrator>,
) -> Result<thread::JoinHandle<()>, Box<dyn std::error::Error>> {
    let repo_name = repo_name.to_string();

    let handle = thread::spawn(move || {
        log::info!("planet {} ({}) thread started", planet_id, repo_name);

        // Create explorer channel
        let (_, rx_explorer) = unbounded::<common_game::protocols::planet_explorer::ExplorerToPlanet>();
        //planet logic
        match repo_name.as_str() {
            "orbitron" => {
                log::info!("start orbitron loop");
                let mut planet= orbitron::create_planet(rx_orchestrator, tx_orchestrator, rx_explorer,planet_id);
                let _ = planet.run();
            }
            "skycartel" => {
                log::info!("start skycartel loop");
                let mut planet= skycartel::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer);
                let _ = planet.run();
            }
            "rustrelli" => {
                log::info!("start rustrelli loop");
                let mut planet = rustrelli::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer, rustrelli::ExplorerRequestLimit::None);
                let _ = planet.run();
            }
            "the_compiler_strikes_back" => {
                log::info!("start the_compiler_strikes_back loop [dummy]");
                //dummy_planet_loop(planet_id, rx_orchestrator, tx_orchestrator);
                //won't start
                let mut planet= the_compiler_strikes_back::planet::create_planet(rx_orchestrator,tx_orchestrator,rx_explorer, planet_id);
                let _ = planet.run();//first planet
            }
            "crabtorio" => {
                log::info!("start crabtorio loop");
                let mut planet= crabtorio::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer);
                let _ = planet.run();
            }
            "houston" => {
                log::info!("start houston loop [dummy]");
                //won't start
                //let mut planet= houston::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer);
                //let _ = planet.run();
                dummy_planet_loop(planet_id, rx_orchestrator, tx_orchestrator);


            }
            "enterprise" => {
                log::info!("start enterprise loop");
                let mut planet= enterprise::create_planet(planet_id, rx_orchestrator, tx_orchestrator, rx_explorer);
                let _ = planet.run();
            }
            // in default use dummy implementation (can't access private fields in real structs)
            _ => {
                log::info!("start dummy loop");
                dummy_planet_loop(planet_id, rx_orchestrator, tx_orchestrator);

            }
        };

        log::info!("planet {} ({}) thread exit", planet_id, repo_name);
    });

    Ok(handle)
}

fn dummy_planet_loop(
    planet_id: u32,
    rx: crossbeam_channel::Receiver<common_game::protocols::orchestrator_planet::OrchestratorToPlanet>,
    tx: crossbeam_channel::Sender<common_game::protocols::orchestrator_planet::PlanetToOrchestrator>,
) {
    // Simple planet implementation for demonstration
    let mut has_rocket = false;
    let mut charged_cells = 0;
    let total_cells = 5;

    // Acknowledge start
    let _ = tx.send(common_game::protocols::orchestrator_planet::PlanetToOrchestrator::StartPlanetAIResult {
        planet_id,
    });

    while let Ok(msg) = rx.recv() {
        match msg {
            common_game::protocols::orchestrator_planet::OrchestratorToPlanet::StartPlanetAI => {
                let _ = tx.send(common_game::protocols::orchestrator_planet::PlanetToOrchestrator::StartPlanetAIResult {
                    planet_id,
                });
            }
            common_game::protocols::orchestrator_planet::OrchestratorToPlanet::Sunray(_) => {
                charged_cells = (charged_cells + 1).min(total_cells);
                let _ = tx.send(common_game::protocols::orchestrator_planet::PlanetToOrchestrator::SunrayAck {
                    planet_id,
                });
                log::debug!("Planet {} received sunray, charged cells: {}/{}", planet_id, charged_cells, total_cells);
            }
            common_game::protocols::orchestrator_planet::OrchestratorToPlanet::Asteroid(_) => {
                let rocket = if has_rocket && charged_cells > 0 {
                    charged_cells -= 1;
                    has_rocket = false;
                    // In real implementation, the planet would build a rocket
                    // For dummy, we just indicate success
                    true
                } else {
                    false
                };

                let _ = tx.send(common_game::protocols::orchestrator_planet::PlanetToOrchestrator::AsteroidAck {
                    planet_id,
                    rocket: if rocket {
                        // Planets should create rockets through their PlanetState
                        // For demo, we return None (indicating no rocket available)
                        None
                    } else {
                        None
                    },
                });

                log::debug!("Planet {} asteroid defense: has_rocket={}, success={}",
                    planet_id, has_rocket, rocket);
            }
            common_game::protocols::orchestrator_planet::OrchestratorToPlanet::InternalStateRequest => {
                let dummy_state = common_game::components::planet::DummyPlanetState {
                    energy_cells: vec![charged_cells > 0; total_cells],
                    charged_cells_count: charged_cells,
                    has_rocket,
                };

                let _ = tx.send(common_game::protocols::orchestrator_planet::PlanetToOrchestrator::InternalStateResponse {
                    planet_id,
                    planet_state: dummy_state,
                });
            }
            common_game::protocols::orchestrator_planet::OrchestratorToPlanet::KillPlanet => {
                let _ = tx.send(common_game::protocols::orchestrator_planet::PlanetToOrchestrator::KillPlanetResult {
                    planet_id,
                });
                break;
            }
            common_game::protocols::orchestrator_planet::OrchestratorToPlanet::IncomingExplorerRequest { explorer_id, new_sender } => {
                log::info!("Planet {} received incoming explorer {}", planet_id, explorer_id);
                let _ = tx.send(common_game::protocols::orchestrator_planet::PlanetToOrchestrator::IncomingExplorerResponse {
                    planet_id,
                    explorer_id,
                    res: Ok(()),
                });
            }
            common_game::protocols::orchestrator_planet::OrchestratorToPlanet::OutgoingExplorerRequest { explorer_id } => {
                log::info!("Planet {} received outgoing explorer {}", planet_id, explorer_id);
                let _ = tx.send(common_game::protocols::orchestrator_planet::PlanetToOrchestrator::OutgoingExplorerResponse {
                    planet_id,
                    explorer_id,
                    res: Ok(()),
                });
            }
            _ => {
                log::debug!("Planet {} received message: {:?}", planet_id, msg);
            }
        }
    }
}

// ==================== EXPLORER SPAWNING ====================

fn spawn_explorers(
    orchestrator: &mut Orchestrator,
    galaxy: &[(u32, Vec<u32>)],
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("spawn explorers...");

    // Explorer names and starting planets (2 explorers as specified)
    let explorers = [
        (100, "alice", 1),
        (101, "bob", 3), //no capitalization because communism
    ];

    for (explorer_id, name, starting_planet) in explorers.iter() {
        // Create communication channels
        let (to_explorer_tx, to_explorer_rx) = unbounded();
        let (from_explorer_tx, from_explorer_rx) = unbounded();

        // Spawn explorer thread
        spawn_explorer_thread(
            *explorer_id,
            *starting_planet,
            name.to_string(),
            to_explorer_rx,
            from_explorer_tx,
        )?;

        // Add to orchestrator
        orchestrator.add_explorer(*explorer_id, to_explorer_tx, from_explorer_rx, *starting_planet)?;

        println!("explorer {} ({}) on planet {}", explorer_id, name, starting_planet);
    }

    Ok(())
}

fn spawn_explorer_thread(
    explorer_id: u32,
    starting_planet: u32,
    name: String,
    rx_orchestrator: crossbeam_channel::Receiver<common_game::protocols::orchestrator_explorer::OrchestratorToExplorer>,
    tx_orchestrator: crossbeam_channel::Sender<common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator<common_game::components::resource::GenericResource>>,
) -> Result<thread::JoinHandle<()>, Box<dyn std::error::Error>> {
    let handle = thread::spawn(move || {
        log::info!("explorer {} ({}) thread started on planet {}", explorer_id, name, starting_planet);

        // Send initial location
        let _ = tx_orchestrator.send(
            common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator::CurrentPlanetResult {
                explorer_id,
                planet_id: starting_planet,
            }
        );

        // Acknowledge start
        let _ = tx_orchestrator.send(
            common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator::StartExplorerAIResult {
                explorer_id,
            }
        );

        // Simple explorer behavior
        let mut current_planet = starting_planet;

        // Request neighbors after a short delay
        let neighbors_tx = tx_orchestrator.clone();
        let explorer_id_clone = explorer_id;
        let current_planet_clone = current_planet;

        // Spawn a thread to request neighbors after startup
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            let _ = neighbors_tx.send(
                common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator::NeighborsRequest {
                    explorer_id: explorer_id_clone,
                    current_planet_id: current_planet_clone,
                }
            );
        });

        while let Ok(msg) = rx_orchestrator.recv() {
            match msg {
                common_game::protocols::orchestrator_explorer::OrchestratorToExplorer::StartExplorerAI => {
                    let _ = tx_orchestrator.send(
                        common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator::StartExplorerAIResult {
                            explorer_id,
                        }
                    );
                }
                common_game::protocols::orchestrator_explorer::OrchestratorToExplorer::KillExplorer => {
                    let _ = tx_orchestrator.send(
                        common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator::KillExplorerResult {
                            explorer_id,
                        }
                    );
                    break;
                }
                common_game::protocols::orchestrator_explorer::OrchestratorToExplorer::MoveToPlanet { planet_id, sender_to_new_planet } => {
                    if planet_id != current_planet && sender_to_new_planet.is_some() {
                        log::info!("explorer {} moving from planet {} to {}", explorer_id, current_planet, planet_id);
                        current_planet = planet_id;
                    }

                    let _ = tx_orchestrator.send(
                        common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator::MovedToPlanetResult {
                            explorer_id,
                            planet_id,
                        }
                    );
                }
                common_game::protocols::orchestrator_explorer::OrchestratorToExplorer::BagContentRequest => {
                    // For BagContentResponse, we need to send a GenericResource
                    // Since we can't create resources directly, we'll send an empty/invalid response
                    // In real implementation, explorers would have actual resources from planets

                    // Send a simple response indicating no resources
                    // We can't create GenericResource directly, so we'll indicate an error
                    // or use a workaround
                    log::warn!("explorer {} received BagContentRequest but cannot create resources in dummy mode", explorer_id);
                }
                common_game::protocols::orchestrator_explorer::OrchestratorToExplorer::CurrentPlanetRequest => {
                    let _ = tx_orchestrator.send(
                        common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator::CurrentPlanetResult {
                            explorer_id,
                            planet_id: current_planet,
                        }
                    );
                }
                common_game::protocols::orchestrator_explorer::OrchestratorToExplorer::NeighborsResponse { neighbors } => {
                    log::debug!("explorer {} received neighbors: {:?}", explorer_id, neighbors);

                    // Auto-travel to first neighbor after receiving neighbors
                    if let Some(&first_neighbor) = neighbors.first() {
                        if first_neighbor != current_planet {
                            thread::sleep(Duration::from_secs(1));
                            let _ = tx_orchestrator.send(
                                common_game::protocols::orchestrator_explorer::ExplorerToOrchestrator::TravelToPlanetRequest {
                                    explorer_id,
                                    current_planet_id: current_planet,
                                    dst_planet_id: first_neighbor,
                                }
                            );
                        }
                    }
                }
                _ => {
                    log::debug!("explorer {} received message: {:?}", explorer_id, msg);
                }
            }
        }

        log::info!("explorer {} thread exiting", explorer_id);
    });

    Ok(handle)
}

// ==================== MONITORING ====================

fn create_monitoring_thread(orchestrator: &Orchestrator) -> crossbeam_channel::Sender<MonitorCommand> {
    let (monitor_tx, monitor_rx) = bounded(10);
    let gui_rx = orchestrator.gui_event_receiver();
    let state = orchestrator.get_state().clone();

    thread::spawn(move || {
        let mut last_update = Instant::now();
        let update_interval = Duration::from_secs(2);

        println!("\nGALAXY MONITOR STARTED");
        println!("=========================");

        // Initial status
        print_status(&state);

        loop {
            // Check for monitor commands
            if let Ok(cmd) = monitor_rx.try_recv() {
                match cmd {
                    MonitorCommand::Status => {
                        print_status(&state);
                    }
                    MonitorCommand::Stats => {
                        print_statistics(&state);
                    }
                    MonitorCommand::Exit => break,
                }
            }

            // Check for GUI events
            if let Ok(event) = gui_rx.try_recv() {
                if let orchestrator_alien::orchestrator::gui_interface::GuiEvent::StateUpdate(state) = event {
                    if last_update.elapsed() >= update_interval {
                        print_live_status(&state);
                        last_update = Instant::now();
                    }
                }
            }

            // Sleep to avoid busy waiting
            thread::sleep(Duration::from_millis(100));
        }

        println!("Galaxy monitor shutting down...");
    });

    monitor_tx
}

fn print_status(state: &SystemState) {
    println!("\nCURRENT GALAXY STATUS");
    println!("=======================");
    println!("Game State: {:?}", state.game_state());
    println!("Active Planets: {}", state.get_alive_planets_sorted().len());
    println!("Active Explorers: {}", state.explorer_locations().len());
    println!("");

    println!("PLANETS:");
    for planet_id in state.get_alive_planets_sorted() {
        let explorers = state.get_explorers_on_planet(planet_id);
        let neighbors = state.get_neighbors(planet_id);
        println!("  Planet {} - Explorers: {}, Neighbors: {:?}",
                 planet_id, explorers.len(), neighbors);
    }

    println!("\nEXPLORERS:");
    for (explorer_id, planet_id) in state.explorer_locations() {
        println!("  Explorer {} on Planet {}", explorer_id, planet_id);
    }
}

fn print_statistics(state: &SystemState) {
    let stats = state.game_stats();
    println!("\nGAME STATISTICS");
    println!("==================");
    println!("Asteroids Sent: {}", stats.asteroids_sent);
    println!("Sunrays Sent: {}", stats.sunrays_sent);
    println!("Planets Destroyed: {}", stats.planets_destroyed);
    println!("Explorers Killed: {}", stats.explorers_killed);
    println!("Resources Generated: {}", stats.resources_generated);

    // Calculate survival rate
    let total_planets = 7; // Starting planets
    let survival_rate = if total_planets > 0 {
        (state.get_alive_planets_sorted().len() as f32 / total_planets as f32) * 100.0
    } else {
        0.0
    };
    println!("Planet Survival Rate: {:.1}%", survival_rate);
}

fn print_live_status(state: &GuiState) {
    println!("\nLIVE UPDATE - {}", chrono::Local::now().format("%H:%M:%S"));
    println!("planets: {}, explorers: {}", state.planets.len(), state.explorers.len());
    println!(
        "galaxy AI: enabled={}, targets={:?}, pending={:?}, attempts={}/2, cooldown_left={}",
        state.galaxy_ai.enabled,
        state.galaxy_ai.target_set,
        state.galaxy_ai.pending_target,
        state.galaxy_ai.attempts_made,
        state.galaxy_ai.cooldown_intervals_remaining
    );

    // Show any explorers that recently moved
    for explorer in &state.explorers {
        println!("  explorer {} on planet {}", explorer.id, explorer.current_planet);
    }
}

// ==================== ORCHESTRATOR THREAD ====================

fn start_orchestrator_thread(mut orchestrator: Orchestrator) -> Result<thread::JoinHandle<Result<(), String>>, Box<dyn std::error::Error>> {
    let handle = thread::spawn(move || {
        log::info!("orchestrator main thread starting...");

        // Send start commands to all planets and explorers after a brief delay
        thread::sleep(Duration::from_secs(1));

        // Start all planet AIs
        for planet_id in orchestrator.state.get_alive_planets_sorted() {
            if let Some(sender) = orchestrator.planet_senders.get(&planet_id) {
                let _ = sender.send(common_game::protocols::orchestrator_planet::OrchestratorToPlanet::StartPlanetAI);
            }
        }

        // Start all explorer AIs
        for explorer_id in orchestrator.explorer_senders.keys() {
            if let Some(sender) = orchestrator.explorer_senders.get(explorer_id) {
                let _ = sender.send(common_game::protocols::orchestrator_explorer::OrchestratorToExplorer::StartExplorerAI);
            }
        }

        // Run the orchestrator
        orchestrator.run()
    });

    Ok(handle)
}

// ==================== INTERACTIVE CONSOLE ====================

fn interactive_console(monitor_tx: crossbeam_channel::Sender<MonitorCommand>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nINTERACTIVE CONSOLE");
    println!("Type commands to control the simulation");
    println!("Commands: status, stats, send_asteroid [planet], send_sunray [planet], pause, resume, quit");
    println!("Example: send_asteroid 3");
    println!("");

    let mut input = String::new();
    let stdin = io::stdin();

    loop {
        print!("galaxy> ");
        io::stdout().flush()?;

        input.clear();
        stdin.read_line(&mut input)?;
        let input = input.trim();

        match input {
            "status" => {
                let _ = monitor_tx.send(MonitorCommand::Status);
            }
            "stats" => {
                let _ = monitor_tx.send(MonitorCommand::Stats);
            }
            "pause" | "p" => {
                println!("simulation paused (command would be sent to orchestrator)");
                // In real implementation: orchestrator.gui_command_sender().send(GuiCommand::PauseSimulation)
            }
            "resume" | "r"=> {
                println!("simulation resumed (command would be sent to orchestrator)");
                // In real implementation: orchestrator.gui_command_sender().send(GuiCommand::ResumeSimulation)
            }
            cmd if cmd.starts_with("send_asteroid ") => {
                if let Some(planet_str) = cmd.strip_prefix("send_asteroid ") {
                    if let Ok(planet_id) = planet_str.parse::<u32>() {
                        println!("Command to send asteroid to planet {} (would be sent to orchestrator)", planet_id);
                        // In real implementation: orchestrator.send_asteroid_to_planet(planet_id)
                    } else {
                        println!("invalid planet ID");
                    }
                }
            }
            cmd if cmd.starts_with("send_sunray ") => {
                if let Some(planet_str) = cmd.strip_prefix("send_sunray ") {
                    if let Ok(planet_id) = planet_str.parse::<u32>() {
                        println!("command to send sunray to planet {} (would be sent to orchestrator)", planet_id);
                        // In real implementation: orchestrator.send_sunray_to_planet(planet_id)
                    } else {
                        println!("invalid planet ID");
                    }
                }
            }
            "quit" | "exit" | "q" => {
                println!("shutting down...");
                let _ = monitor_tx.send(MonitorCommand::Exit);
                std::process::exit(0);
                break;
            }
            "help" | "h"=> {
                println!("Available commands:");
                println!("  status           - Show current galaxy status");
                println!("  stats            - Show game statistics");
                println!("  send_asteroid N  - Send asteroid to planet N");
                println!("  send_sunray N    - Send sunray to planet N");
                println!("  pause            - Pause simulation");
                println!("  resume           - Resume simulation");
                println!("  quit/exit        - Exit simulation");
            }
            "" => continue,
            _ => {
                println!("unknown command. Type 'help' for available commands.");
            }
        }
    }

    Ok(())
}

// ==================== TYPES ====================

enum MonitorCommand {
    Status,
    Stats,
    Exit,
}