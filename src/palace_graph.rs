use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

use crate::storage::PalaceStore;
use crate::Result;

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub wings: Vec<String>,
    pub halls: Vec<String>,
    pub count: usize,
    pub dates: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub room: String,
    pub wing_a: String,
    pub wing_b: String,
    pub hall: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraversedNode {
    pub room: String,
    pub wings: Vec<String>,
    pub halls: Vec<String>,
    pub count: usize,
    pub hop: usize,
    pub connected_via: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TunnelRoom {
    pub room: String,
    pub wings: Vec<String>,
    pub halls: Vec<String>,
    pub count: usize,
    pub recent: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PalaceGraphStats {
    pub total_rooms: usize,
    pub tunnel_rooms: usize,
    pub total_edges: usize,
    pub rooms_per_wing: BTreeMap<String, usize>,
    pub top_tunnels: Vec<TunnelRoom>,
}

pub fn build_graph(store: &PalaceStore) -> Result<(BTreeMap<String, GraphNode>, Vec<GraphEdge>)> {
    let drawers = store.list_drawers(None, None)?;
    let mut room_data = BTreeMap::<String, (BTreeSet<String>, BTreeSet<String>, usize, BTreeSet<String>)>::new();
    for drawer in drawers {
        if drawer.room == "general" {
            continue;
        }
        let entry = room_data
            .entry(drawer.room.clone())
            .or_insert_with(|| (BTreeSet::new(), BTreeSet::new(), 0, BTreeSet::new()));
        entry.0.insert(drawer.wing.clone());
        if let Some(hall) = drawer.hall.clone() {
            entry.1.insert(hall);
        }
        if let Some(date) = drawer.date.clone() {
            entry.3.insert(date);
        } else if !drawer.filed_at.is_empty() {
            entry.3.insert(drawer.filed_at.clone());
        }
        entry.2 += 1;
    }

    let mut nodes = BTreeMap::new();
    let mut edges = Vec::new();
    for (room, (wings, halls, count, dates)) in room_data {
        let wings_vec = wings.iter().cloned().collect::<Vec<_>>();
        let halls_vec = halls.iter().cloned().collect::<Vec<_>>();
        let dates_vec = dates.iter().cloned().collect::<Vec<_>>();
        if wings_vec.len() >= 2 {
            for (index, wing_a) in wings_vec.iter().enumerate() {
                for wing_b in wings_vec.iter().skip(index + 1) {
                    if halls_vec.is_empty() {
                        edges.push(GraphEdge {
                            room: room.clone(),
                            wing_a: wing_a.clone(),
                            wing_b: wing_b.clone(),
                            hall: String::new(),
                            count,
                        });
                    } else {
                        for hall in &halls_vec {
                            edges.push(GraphEdge {
                                room: room.clone(),
                                wing_a: wing_a.clone(),
                                wing_b: wing_b.clone(),
                                hall: hall.clone(),
                                count,
                            });
                        }
                    }
                }
            }
        }
        nodes.insert(
            room,
            GraphNode {
                wings: wings_vec,
                halls: halls_vec,
                count,
                dates: dates_vec.into_iter().rev().take(5).collect(),
            },
        );
    }

    Ok((nodes, edges))
}

pub fn traverse(store: &PalaceStore, start_room: &str, max_hops: usize) -> Result<Vec<TraversedNode>> {
    let (nodes, _) = build_graph(store)?;
    let Some(start) = nodes.get(start_room) else {
        return Ok(Vec::new());
    };

    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    let mut results = Vec::new();
    visited.insert(start_room.to_string());
    queue.push_back((start_room.to_string(), 0usize));
    results.push(TraversedNode {
        room: start_room.to_string(),
        wings: start.wings.clone(),
        halls: start.halls.clone(),
        count: start.count,
        hop: 0,
        connected_via: Vec::new(),
    });

    while let Some((room_name, depth)) = queue.pop_front() {
        if depth >= max_hops {
            continue;
        }
        let current = nodes.get(&room_name).unwrap();
        let current_wings = current.wings.iter().cloned().collect::<BTreeSet<_>>();
        for (candidate_room, candidate) in &nodes {
            if visited.contains(candidate_room) {
                continue;
            }
            let shared = candidate
                .wings
                .iter()
                .filter(|wing| current_wings.contains(*wing))
                .cloned()
                .collect::<Vec<_>>();
            if shared.is_empty() {
                continue;
            }
            visited.insert(candidate_room.clone());
            results.push(TraversedNode {
                room: candidate_room.clone(),
                wings: candidate.wings.clone(),
                halls: candidate.halls.clone(),
                count: candidate.count,
                hop: depth + 1,
                connected_via: shared.clone(),
            });
            if depth + 1 < max_hops {
                queue.push_back((candidate_room.clone(), depth + 1));
            }
        }
    }

    results.sort_by(|left, right| left.hop.cmp(&right.hop).then_with(|| right.count.cmp(&left.count)));
    Ok(results)
}

pub fn find_tunnels(
    store: &PalaceStore,
    wing_a: Option<&str>,
    wing_b: Option<&str>,
) -> Result<Vec<TunnelRoom>> {
    let (nodes, _) = build_graph(store)?;
    let mut tunnels = nodes
        .into_iter()
        .filter_map(|(room, node)| {
            if node.wings.len() < 2 {
                return None;
            }
            if let Some(wing_a) = wing_a {
                if !node.wings.iter().any(|wing| wing == wing_a) {
                    return None;
                }
            }
            if let Some(wing_b) = wing_b {
                if !node.wings.iter().any(|wing| wing == wing_b) {
                    return None;
                }
            }
            Some(TunnelRoom {
                room,
                wings: node.wings,
                halls: node.halls,
                count: node.count,
                recent: node.dates.last().cloned().unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    tunnels.sort_by(|left, right| right.count.cmp(&left.count).then_with(|| left.room.cmp(&right.room)));
    Ok(tunnels)
}

pub fn graph_stats(store: &PalaceStore) -> Result<PalaceGraphStats> {
    let (nodes, edges) = build_graph(store)?;
    let mut rooms_per_wing = BTreeMap::<String, usize>::new();
    for node in nodes.values() {
        for wing in &node.wings {
            *rooms_per_wing.entry(wing.clone()).or_default() += 1;
        }
    }
    let mut top_tunnels = find_tunnels(store, None, None)?;
    top_tunnels.truncate(10);
    Ok(PalaceGraphStats {
        total_rooms: nodes.len(),
        tunnel_rooms: nodes.values().filter(|node| node.wings.len() >= 2).count(),
        total_edges: edges.len(),
        rooms_per_wing,
        top_tunnels,
    })
}
