use std::collections::{HashMap, VecDeque};

use crate::data::{GalaxyData, SystemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum RouteKind {
    Major,
    Minor,
}

pub type RouteKey = (SystemId, SystemId);

pub fn route_key(a: SystemId, b: SystemId) -> RouteKey {
    if a <= b { (a, b) } else { (b, a) }
}

pub fn build_adjacency(galaxy: &GalaxyData) -> HashMap<SystemId, Vec<SystemId>> {
    let mut adjacency = HashMap::<SystemId, Vec<SystemId>>::new();
    for system in &galaxy.systems {
        adjacency.entry(system.id).or_default();
    }
    for route in &galaxy.routes {
        adjacency.entry(route.a).or_default().push(route.b);
        adjacency.entry(route.b).or_default().push(route.a);
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort();
        neighbors.dedup();
    }
    adjacency
}

pub fn bfs_distances(
    adjacency: &HashMap<SystemId, Vec<SystemId>>,
    sources: &[SystemId],
) -> HashMap<SystemId, u32> {
    let mut distances = HashMap::<SystemId, u32>::new();
    let mut queue = VecDeque::new();
    for source in sources {
        if distances.insert(*source, 0).is_none() {
            queue.push_back(*source);
        }
    }

    while let Some(current) = queue.pop_front() {
        let distance = distances[&current];
        let Some(neighbors) = adjacency.get(&current) else {
            continue;
        };
        for neighbor in neighbors {
            if distances.contains_key(neighbor) {
                continue;
            }
            distances.insert(*neighbor, distance + 1);
            queue.push_back(*neighbor);
        }
    }
    distances
}

pub fn shortest_path(
    adjacency: &HashMap<SystemId, Vec<SystemId>>,
    start: SystemId,
    goal: SystemId,
) -> Option<Vec<SystemId>> {
    let mut previous = HashMap::<SystemId, SystemId>::new();
    let mut seen = HashMap::<SystemId, u32>::new();
    let mut queue = VecDeque::new();
    seen.insert(start, 0);
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        if current == goal {
            let mut path = vec![goal];
            let mut cursor = goal;
            while cursor != start {
                cursor = previous[&cursor];
                path.push(cursor);
            }
            path.reverse();
            return Some(path);
        }
        let Some(neighbors) = adjacency.get(&current) else {
            continue;
        };
        for neighbor in neighbors {
            if seen.contains_key(neighbor) {
                continue;
            }
            seen.insert(*neighbor, seen[&current] + 1);
            previous.insert(*neighbor, current);
            queue.push_back(*neighbor);
        }
    }
    None
}
