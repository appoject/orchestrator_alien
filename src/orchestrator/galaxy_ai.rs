//! Galaxy AI that decides when to send sunrays and asteroids.
//!
//! Mechanic:
//! - A sunray is sent to a random alive planet on a cadence set by
//!   `sunray_scale_cycles`, which slows down sharply as the galaxy shrinks
//!   so survivors don't out-heal a thinning attack pool.
//! - The AI keeps a "target set" of up to 3 consecutive alive planets
//!   (consecutive = adjacent positions in the sorted list of currently
//!   alive planet IDs, wrapping around if the start is near the end).
//! - Against the current target set the AI performs exactly two asteroid
//!   attempts, each aimed at a uniformly random *currently alive* member
//!   of the set. After each attempt the AI goes silent on asteroids (but
//!   keeps sending sunrays) for a fixed number of time intervals depending
//!   on whether the attempt destroyed the planet or was defended, and on
//!   whether it was the first or second attempt:
//!
//!     attempt 1: destroyed -> 3 intervals, defended -> 6 intervals
//!     attempt 2: destroyed -> 8 intervals, defended -> 10 intervals
//!
//!   After the second attempt's cooldown elapses, a brand new target set
//!   is selected. These interval counts are NOT rescaled by alive-planet
//!   count - only the sunray cadence is, since that's what was letting
//!   survivors fully recharge indefinitely.
//! - `scale_cycles` (asteroid pacing) and `sunray_scale_cycles` (sunray
//!   pacing) are deliberately different functions - see each for why.
//!
//! Because asteroid outcomes are only known asynchronously (a planet acks
//! later via `PlanetToOrchestrator::AsteroidAck`), this AI does not decide
//! outcomes itself. The orchestrator must call [`GalaxyAI::notify_asteroid_result`]
//! whenever such an ack arrives. If no ack ever arrives (e.g. a buggy
//! planet implementation), [`GalaxyAI::update`] self-heals via a timeout
//! so the campaign can't stall forever.

use common_game::utils::ID;
use rand::seq::{IndexedRandom, SliceRandom};
use rand::Rng;

/// Actions that the Galaxy AI can request the orchestrator perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GalaxyAction {
    /// Send an asteroid to attack a planet
    SendAsteroid { target_planet: ID },
    /// Send a sunray to help a planet
    SendSunray { target_planet: ID },
}

/// Internal asteroid-campaign state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsteroidState {
    /// No usable target set yet; need to pick one.
    SelectingTargets,
    /// Have a target set, no shot pending, ready to fire the next attempt.
    ReadyToAttack,
    /// An asteroid was fired at `pending_target`; waiting for its ack.
    WaitingForResult,
    /// Between attempts (or after the second attempt): sunrays only.
    Cooldown,
}

/// Galaxy AI that decides which planets get sunrays or asteroids.
#[derive(Debug, Clone)]
pub struct GalaxyAI {
    enabled: bool,

    // ---- asteroid campaign state ----
    asteroid_state: AsteroidState,
    /// Up to 3 planet IDs currently designated as the campaign's targets.
    target_set: Vec<ID>,
    /// How many asteroid attempts have been resolved against `target_set` (0, 1 or 2).
    attempts_made: u8,
    /// The planet an in-flight asteroid was sent to, awaiting its ack.
    pending_target: Option<ID>,
    /// Time intervals still left in the current cooldown.
    cooldown_intervals_remaining: u32,
    /// Cycles elapsed within the interval currently being counted down.
    cooldown_cycle_progress: u32,
    /// Cycles spent waiting for the current pending shot's ack.
    waiting_cycle_progress: u32,

    // ---- sunray heartbeat state ----
    /// Cycles elapsed within the current sunray interval.
    sunray_cycle_progress: u32,
}

impl GalaxyAI {
    /// If an asteroid ack doesn't arrive within this many engine cycles, the
    /// shot is treated as lost/unresponsive so the campaign can keep moving
    /// instead of stalling forever (protects against a planet impl that
    /// never acks).
    const ASTEROID_ACK_TIMEOUT_CYCLES: u32 = 50;

    /// Creates a new Galaxy AI that will NOT act until enabled.
    #[must_use]
    pub fn new_inactive() -> Self {
        Self {
            enabled: false,
            asteroid_state: AsteroidState::SelectingTargets,
            target_set: Vec::new(),
            attempts_made: 0,
            pending_target: None,
            cooldown_intervals_remaining: 0,
            cooldown_cycle_progress: 0,
            waiting_cycle_progress: 0,
            sunray_cycle_progress: 0,
        }
    }

    /// Creates a new Galaxy AI that is active immediately.
    #[must_use]
    pub fn new_active() -> Self {
        let mut ai = Self::new_inactive();
        ai.enabled = true;
        ai
    }

    /// Enable the AI.
    pub fn enable_ai(&mut self) {
        self.enabled = true;
    }

    /// Disable the AI. In-flight campaign state is preserved so it resumes
    /// where it left off if re-enabled.
    pub fn disable_ai(&mut self) {
        self.enabled = false;
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Current target set (up to 3 planet IDs), empty if not yet selected.
    #[must_use]
    pub fn get_target_set(&self) -> &[ID] {
        &self.target_set
    }

    /// How many of the 2 attempts against the current set have resolved.
    #[must_use]
    pub fn get_attempts_made(&self) -> u8 {
        self.attempts_made
    }

    /// Planet an in-flight asteroid is currently aimed at, if any.
    #[must_use]
    pub fn get_pending_target(&self) -> Option<ID> {
        self.pending_target
    }

    /// Time intervals left in the current sunray-only cooldown (0 if not cooling down).
    #[must_use]
    pub fn get_cooldown_intervals_remaining(&self) -> u32 {
        self.cooldown_intervals_remaining
    }

    /// Cycles between asteroid-campaign pacing ticks (attack cadence,
    /// cooldown countdown speed). Deliberately mild - the *interval counts*
    /// (3/6/8/10) already do most of the endgame pacing work.
    fn scale_cycles(alive_count: usize) -> u32 {
        match alive_count {
            6..=7 => 1,
            4..=5 => 2,
            2..=3 => 3,
            0..=1 => 4,
            _ => 1,
        }
    }

    /// Cycles between sunray heartbeats. This must fall off much faster
    /// than `scale_cycles` as the galaxy shrinks: with fewer alive planets,
    /// any single planet has a much higher chance of being the one a
    /// random sunray lands on, so firing needs to slow down
    /// disproportionately or survivors can recharge a rocket before every
    /// single asteroid attempt, and the campaign never concludes.
    fn sunray_scale_cycles(alive_count: usize) -> u32 {
        match alive_count {
            6..=7 => 1,
            4..=5 => 4,
            2..=3 => 12,
            0..=1 => 20,
            _ => 1,
        }
    }

    /// Picks up to 3 consecutive alive planets (consecutive in the sorted
    /// list of currently alive planet IDs, wrapping around).
    fn select_target_set(alive_planets_sorted: &[ID]) -> Vec<ID> {
        let len = alive_planets_sorted.len();
        if len == 0 {
            return Vec::new();
        }
        let take = len.min(3);
        let mut rng = rand::thread_rng();
        let start = rng.gen_range(0..len);
        (0..take)
            .map(|offset| alive_planets_sorted[(start + offset) % len])
            .collect()
    }

    /// Picks a uniformly random member of `target_set` that is still alive.
    fn pick_alive_target(&self, alive_planets: &[ID]) -> Option<ID> {
        let alive_members: Vec<ID> = self
            .target_set
            .iter()
            .copied()
            .filter(|id| alive_planets.contains(id))
            .collect();
        let mut rng = rand::thread_rng();
        alive_members.choose(&mut rng).copied()
    }

    /// Called once per engine cycle. Returns the actions the orchestrator
    /// should carry out this cycle (a sunray and an asteroid can both fire
    /// on the same cycle).
    pub fn update(&mut self, alive_planets_sorted: &[ID]) -> Vec<GalaxyAction> {
        let mut actions = Vec::new();

        if !self.enabled || alive_planets_sorted.is_empty() {
            return actions;
        }

        // ---- sunray heartbeat: fires once per (harshly scaled) interval ----
        let sunray_interval_cycles = Self::sunray_scale_cycles(alive_planets_sorted.len());
        self.sunray_cycle_progress += 1;
        if self.sunray_cycle_progress >= sunray_interval_cycles {
            self.sunray_cycle_progress = 0;
            let mut rng = rand::thread_rng();
            if let Some(&target) = alive_planets_sorted.choose(&mut rng) {
                actions.push(GalaxyAction::SendSunray { target_planet: target });
            }
        }

        // ---- asteroid campaign ----
        match self.asteroid_state {
            AsteroidState::SelectingTargets => {
                self.target_set = Self::select_target_set(alive_planets_sorted);
                self.attempts_made = 0;
                if !self.target_set.is_empty() {
                    log::info!("Galaxy AI selected new target set: {:?}", self.target_set);
                    self.asteroid_state = AsteroidState::ReadyToAttack;
                }
                // else: nothing alive to target; retry next cycle
            }
            AsteroidState::ReadyToAttack => match self.pick_alive_target(alive_planets_sorted) {
                Some(target) => {
                    log::info!(
                        "Galaxy AI attacking planet {target} (attempt {})",
                        self.attempts_made + 1
                    );
                    actions.push(GalaxyAction::SendAsteroid { target_planet: target });
                    self.pending_target = Some(target);
                    self.waiting_cycle_progress = 0;
                    self.asteroid_state = AsteroidState::WaitingForResult;
                }
                None => {
                    // Every member of the set is already dead (e.g. via a
                    // manual GUI kill) - pick a new set.
                    self.asteroid_state = AsteroidState::SelectingTargets;
                }
            },
            AsteroidState::WaitingForResult => {
                self.waiting_cycle_progress += 1;
                if self.waiting_cycle_progress >= Self::ASTEROID_ACK_TIMEOUT_CYCLES {
                    let stuck_planet = self.pending_target;
                    log::warn!(
                        "Galaxy AI: no asteroid ack from planet {:?} after {} cycles \
                         (planet implementation likely didn't respond) - dropping it \
                         from the target set and moving on",
                        stuck_planet, self.waiting_cycle_progress
                    );
                    if let Some(id) = stuck_planet {
                        self.target_set.retain(|&p| p != id);
                    }
                    self.pending_target = None;
                    self.attempts_made += 1;
                    self.waiting_cycle_progress = 0;
                    self.asteroid_state = if self.attempts_made >= 2 || self.target_set.is_empty() {
                        AsteroidState::SelectingTargets
                    } else {
                        AsteroidState::ReadyToAttack
                    };
                }
            }
            AsteroidState::Cooldown => {
                let interval_cycles = Self::scale_cycles(alive_planets_sorted.len());
                self.cooldown_cycle_progress += 1;
                if self.cooldown_cycle_progress >= interval_cycles {
                    self.cooldown_cycle_progress = 0;
                    self.cooldown_intervals_remaining =
                        self.cooldown_intervals_remaining.saturating_sub(1);
                    if self.cooldown_intervals_remaining == 0 {
                        self.asteroid_state = if self.attempts_made >= 2 {
                            AsteroidState::SelectingTargets
                        } else {
                            AsteroidState::ReadyToAttack
                        };
                    }
                }
            }
        }

        actions
    }

    /// Must be called whenever a planet acks an asteroid, so the AI can
    /// learn the outcome of an attempt it made. Acks for asteroids that did
    /// not originate from this AI (e.g. GUI-triggered) are ignored.
    pub fn notify_asteroid_result(&mut self, planet_id: ID, destroyed: bool) {
        if self.pending_target != Some(planet_id) {
            return;
        }
        self.pending_target = None;
        self.attempts_made += 1;

        self.cooldown_intervals_remaining = match (self.attempts_made, destroyed) {
            (1, true) => 3,
            (1, false) => 6,
            (2, true) => 8,
            (2, false) => 10,
            _ => 6, // defensive default, should not be reached
        };
        self.cooldown_cycle_progress = 0;
        self.asteroid_state = AsteroidState::Cooldown;

        log::info!(
            "Galaxy AI: planet {planet_id} {} on attempt {}/2; cooldown for {} intervals",
            if destroyed { "was destroyed" } else { "defended" },
            self.attempts_made,
            self.cooldown_intervals_remaining
        );
    }

    /// Must be called whenever a planet is removed from the galaxy through
    /// any path other than a normal asteroid ack (disconnection, GUI kill,
    /// destruction chain, etc.), so the AI never stalls forever waiting for
    /// an ack that will never arrive.
    pub fn notify_planet_removed(&mut self, planet_id: ID) {
        self.target_set.retain(|&id| id != planet_id);

        if self.pending_target == Some(planet_id) {
            self.pending_target = None;
            self.attempts_made += 1;
            self.cooldown_intervals_remaining = if self.attempts_made <= 1 { 3 } else { 8 };
            self.cooldown_cycle_progress = 0;
            self.asteroid_state = AsteroidState::Cooldown;

            log::warn!(
                "Galaxy AI: planet {planet_id} removed while an asteroid was pending; \
                 forcing cooldown to avoid a stall"
            );
        }
    }
}