use std::collections::HashSet;

use crate::data::{GalaxyRoute, StarSystemData};

pub fn generate_routes(systems: &[StarSystemData]) -> Vec<GalaxyRoute> {
    let mut links = HashSet::new();
    let mut degree = vec![0_usize; systems.len()];

    for (index, system) in systems.iter().enumerate() {
        let mut candidates: Vec<(usize, f32)> = systems
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .map(|(other_index, other)| {
                (
                    other_index,
                    system.position.distance_squared(other.position),
                )
            })
            .collect();

        candidates.sort_by(|a, b| a.1.total_cmp(&b.1));

        for (other_index, _) in candidates.into_iter().take(8) {
            if degree[index] >= 6 {
                break;
            }
            if degree[other_index] >= 6 {
                continue;
            }

            let a = systems[index].id;
            let b = systems[other_index].id;
            let key = if a <= b { (a, b) } else { (b, a) };
            if links.insert(key) {
                degree[index] += 1;
                degree[other_index] += 1;
            }
            if degree[index] >= 3 {
                break;
            }
        }
    }

    insert_demo_link(&mut links, systems, 0, 1);
    insert_demo_link(&mut links, systems, 1, 2);

    let mut routes: Vec<_> = links
        .into_iter()
        .map(|(a, b)| GalaxyRoute { a, b })
        .collect();
    routes.sort_by_key(|route| (route.a.0, route.b.0));
    routes
}

fn insert_demo_link(
    links: &mut HashSet<(crate::data::SystemId, crate::data::SystemId)>,
    systems: &[StarSystemData],
    a_index: usize,
    b_index: usize,
) {
    let (Some(a), Some(b)) = (systems.get(a_index), systems.get(b_index)) else {
        return;
    };
    let key = if a.id <= b.id {
        (a.id, b.id)
    } else {
        (b.id, a.id)
    };
    links.insert(key);
}
