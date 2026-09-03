//! Galaxy topology management.

use std::collections::{HashMap, HashSet};
use common_game::utils::ID;

/// Represents the galaxy topology (planets and their connections)
#[derive(Debug, Clone)]
pub struct GalaxyStructure {
    adjacency: HashMap<ID, HashSet<ID>>, // 2-way connections between planets
    alive_planets: HashSet<ID>,
}

impl GalaxyStructure {
    /// Creates a new empty galaxy
    #[must_use]
    pub fn new() -> Self {
        Self {
            adjacency: HashMap::new(),
            alive_planets: HashSet::new(),
        }
    }

    /// Creates a new galaxy from an array of strings
    pub fn new_from_file(structure_file: &[String]) -> Self {
        let mut result = Self::new();

        for line in structure_file {
            let parsed = Self::parse_u32_with_errors(line);

            if !parsed.is_empty() {
                let planet = parsed[0] as ID;
                let adjacents: HashSet<ID> = parsed[1..].iter().map(|&id| id as ID).collect();
                result.add_planet_unchecked(planet, adjacents);
            } else {
                log::error!("File is either empty or invalid: '{}'", line);
            }
        }

        result
    }

    /// Parses a string into u32 IDs
    fn parse_u32_with_errors(input: &str) -> Vec<u32> {
        let res: Result<Vec<u32>, String> = input
            .split_whitespace()
            .map(|s| s.parse::<u32>().map_err(|e| format!("'{}': {}", s, e)))
            .collect();

        match res {
            Ok(vec) => vec,
            Err(e) => {
                log::error!("Failed to parse input: {}. Returning empty vector.", e);
                vec![]
            }
        }
    }

    /// Adds a planet without checking adjacents
    fn add_planet_unchecked(&mut self, planet: ID, adjacents: HashSet<ID>) {
        self.alive_planets.insert(planet);
        self.adjacency.insert(planet, adjacents);
    }

    /// Adds a planet and connects it to adjacents
    pub fn add_planet(&mut self, planet: ID, adjacents: &[ID]) {
        self.alive_planets.insert(planet);
        let set: HashSet<ID> = adjacents.iter().copied().collect();
        self.adjacency.insert(planet, set);

        // Update the adjacency of existing planets
        for &a in adjacents {
            self.adjacency.entry(a).or_default().insert(planet);
        }
    }

    /// Removes a planet from the galaxy
    pub fn remove_planet(&mut self, id: ID) {
        self.adjacency.remove(&id);
        for connections in self.adjacency.values_mut() {
            connections.remove(&id);
        }
        self.alive_planets.remove(&id);


        //log change in galaxy structure
        log::info!(
            "New galaxy structure is: \n{}", self.print()
        );
    }
    fn print(&self) -> String {
        let mut ids: Vec<&ID> = self.adjacency.keys().collect();
        ids.sort();

        let mut output = String::new();
        for id in ids {
            let mut connections: Vec<&ID> = self.adjacency[id].iter().collect();
            connections.sort();
            let conn_str = connections.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ");
            output.push_str(&format!("P {:>3} -> [{}]\n", id, conn_str));
        }
        output
    }

    /// Adds a connection between two planets
    pub fn add_connection(&mut self, a: ID, b: ID) {
        if !self.is_alive(a) || !self.is_alive(b) {
            log::error!(
                "Cannot connect dead planets: planet {a} is {}, planet {b} is {}",
                if self.is_alive(a) { "alive" } else { "dead" },
                if self.is_alive(b) { "alive" } else { "dead" }
            );
        } else {
            self.adjacency.entry(a).or_default().insert(b);
            self.adjacency.entry(b).or_default().insert(a);
        }
    }

    ///Adds a connection between two planets without checking
    pub fn add_connection_unchecked(&mut self, a: ID, b: ID) {
        self.adjacency.entry(a).or_default().insert(b);
        self.adjacency.entry(b).or_default().insert(a);
    }

    /// Checks if travel is possible from one planet to another
    #[must_use]
    pub fn can_travel(&self, from: ID, to: ID) -> bool {
        self.is_alive(to)
            && self.adjacency.get(&from).map_or(false, |n| n.contains(&to))
    }

    /// Checks if a planet is alive
    #[must_use]
    pub fn is_alive(&self, planet: ID) -> bool {
        self.alive_planets.contains(&planet)
    }

    /// Returns all alive planets
    #[must_use]
    pub fn get_alive_planets(&self) -> &HashSet<ID> {
        &self.alive_planets
    }

    /// Returns the adjacency map
    #[must_use]
    pub fn get_adjacency(&self) -> &HashMap<ID, HashSet<ID>> {
        &self.adjacency
    }

    /// Returns the adjacents to a specific planet
    #[must_use]
    pub fn get_adjacents(&self, planet: ID) -> HashSet<ID> {
        self.adjacency.get(&planet).cloned().unwrap_or_default()
    }
}

impl Default for GalaxyStructure {
    fn default() -> Self {
        Self::new()
    }
}

