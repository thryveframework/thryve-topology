//! Placing the call sites: columns from left to right, so that an edge points to a later column
//! wherever the graph allows it. Each separate part of the graph gets its own band, one under the
//! other, in the file's order.
//!
//! The steps, for each part:
//! 1. the starting points (no incoming edge) are walked depth first; an edge back to a call site
//!    still on the walk's path closes a loop, and is left out of the column count;
//! 2. each call site's column is one more than the furthest column that edges to it;
//! 3. an edge that skips columns gets a waypoint in each column it crosses, so it's routed through
//!    a gap between cards instead of under them;
//! 4. within a column, cards and waypoints are ordered by where their neighbours sit, a few times
//!    over in both directions, so fewer edges cross;
//! 5. columns are as wide as their widest card and centred on the tallest column.

use std::collections::{HashMap, HashSet};

use egui::{Pos2, Vec2, pos2, vec2};

/// The space between two columns, for the arrows and their counts.
pub const COLUMN_GAP: f32 = 150.0;
/// The space between two cards in a column.
pub const ROW_GAP: f32 = 50.0;
/// The space between two separate parts of the graph.
pub const PART_GAP: f32 = 140.0;
/// The room a waypoint takes in its column.
const WAYPOINT_HEIGHT: f32 = 18.0;

/// Where everything goes, in world units.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Arrangement {
    /// Each card's centre.
    pub centers: Vec<Pos2>,
    /// For each edge `(from, to)` that skips columns, the waypoints it passes through, in order.
    pub routes: HashMap<(usize, usize), Vec<Pos2>>,
}

/// Places cards of the given `sizes`, connected by `(from, to)` pairs. Edges from a card to itself
/// are ignored.
pub fn arrange(sizes: &[Vec2], edges: &[(usize, usize)]) -> Arrangement {
    let n = sizes.len();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut into: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(from, to) in edges {
        if from != to && from < n && to < n && !out[from].contains(&to) {
            out[from].push(to);
            into[to].push(from);
        }
    }

    let mut arrangement = Arrangement {
        centers: vec![Pos2::ZERO; n],
        routes: HashMap::new(),
    };
    let mut top = 0.0;
    for part in parts(n, &out, &into) {
        let placed = arrange_part(&part, sizes, &out, &into);
        let shift = vec2(0.0, top);
        for (site, center) in placed.centers {
            arrangement.centers[site] = center + shift;
        }
        for (edge, waypoints) in placed.routes {
            arrangement
                .routes
                .insert(edge, waypoints.into_iter().map(|p| p + shift).collect());
        }
        top += placed.height + PART_GAP;
    }
    arrangement
}

/// The weakly connected parts, each sorted, in the order of their first call site.
fn parts(n: usize, out: &[Vec<usize>], into: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut part_of = vec![usize::MAX; n];
    let mut parts = Vec::new();
    for first in 0..n {
        if part_of[first] != usize::MAX {
            continue;
        }

        let id = parts.len();
        let mut members = vec![first];
        part_of[first] = id;
        let mut stack = vec![first];
        while let Some(site) = stack.pop() {
            for &next in out[site].iter().chain(&into[site]) {
                if part_of[next] == usize::MAX {
                    part_of[next] = id;
                    members.push(next);
                    stack.push(next);
                }
            }
        }
        members.sort_unstable();
        parts.push(members);
    }
    parts
}

/// One placed part, with its top at y = 0.
struct PlacedPart {
    centers: Vec<(usize, Pos2)>,
    routes: Vec<((usize, usize), Vec<Pos2>)>,
    height: f32,
}

/// A slot in a column: a card, or a waypoint of an edge that skips columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Slot {
    Card(usize),
    Waypoint(usize),
}

/// Places one part with its top at y = 0.
fn arrange_part(
    members: &[usize],
    sizes: &[Vec2],
    out: &[Vec<usize>],
    into: &[Vec<usize>],
) -> PlacedPart {
    // 1. Walk depth first from the starting points, noting the order and the edges that close
    // a loop. A part made only of loops starts from its first call site.
    let mut on_path = HashSet::new();
    let mut done = HashSet::new();
    let mut order = Vec::with_capacity(members.len());
    let mut loop_edges = HashSet::new();

    let starts: Vec<usize> = members
        .iter()
        .copied()
        .filter(|&site| into[site].is_empty())
        .collect();
    let rest = members
        .iter()
        .copied()
        .filter(|site| !starts.contains(site));
    for root in starts.iter().copied().chain(rest) {
        if on_path.contains(&root) || done.contains(&root) {
            continue;
        }
        on_path.insert(root);
        order.push(root);
        let mut stack = vec![(root, 0usize)];
        while let Some((site, next_child)) = stack.pop() {
            if let Some(&child) = out[site].get(next_child) {
                stack.push((site, next_child + 1));
                if on_path.contains(&child) {
                    loop_edges.insert((site, child));
                } else if !done.contains(&child) {
                    on_path.insert(child);
                    order.push(child);
                    stack.push((child, 0));
                }
            } else {
                on_path.remove(&site);
                done.insert(site);
            }
        }
    }
    let forward = |from: usize, to: usize| !loop_edges.contains(&(from, to));

    // 2. Columns: the longest way in, over the edges that don't close a loop. The walk's order
    // isn't a topological one, so the columns are settled by repeating until nothing moves.
    let mut column: HashMap<usize, usize> = members.iter().map(|&site| (site, 0)).collect();
    let mut changed = true;
    let mut rounds = 0;
    while changed && rounds <= members.len() {
        changed = false;
        rounds += 1;
        for &site in &order {
            for &next in &out[site] {
                if forward(site, next) && column[&next] < column[&site] + 1 {
                    column.insert(next, column[&site] + 1);
                    changed = true;
                }
            }
        }
    }

    // 3. The slots: every card, and a waypoint in each column a long edge crosses. Edges between
    // slots are what the ordering looks at.
    let column_count = column.values().copied().max().unwrap_or(0) + 1;
    let mut columns: Vec<Vec<Slot>> = vec![Vec::new(); column_count];
    let mut slot_out: HashMap<Slot, Vec<Slot>> = HashMap::new();
    let mut slot_in: HashMap<Slot, Vec<Slot>> = HashMap::new();
    let mut connect = |a: Slot, b: Slot| {
        slot_out.entry(a).or_default().push(b);
        slot_in.entry(b).or_default().push(a);
    };
    for &site in &order {
        columns[column[&site]].push(Slot::Card(site));
    }

    let mut long_edges: Vec<((usize, usize), Vec<Slot>)> = Vec::new();
    let mut waypoint_count = 0;
    for &site in &order {
        for &next in &out[site] {
            if !forward(site, next) {
                continue;
            }
            let (first, last) = (column[&site], column[&next]);
            let mut previous = Slot::Card(site);
            let mut waypoints = Vec::new();
            for crossed in columns.iter_mut().take(last).skip(first + 1) {
                let waypoint = Slot::Waypoint(waypoint_count);
                waypoint_count += 1;
                crossed.push(waypoint);
                connect(previous, waypoint);
                waypoints.push(waypoint);
                previous = waypoint;
            }
            connect(previous, Slot::Card(next));
            if !waypoints.is_empty() {
                long_edges.push(((site, next), waypoints));
            }
        }
    }

    // 4. Order each column by where its neighbours sit, sweeping right, then left.
    let mut row: HashMap<Slot, f32> = HashMap::new();
    let renumber = |columns: &[Vec<Slot>], row: &mut HashMap<Slot, f32>| {
        for col in columns {
            for (i, &slot) in col.iter().enumerate() {
                row.insert(slot, i as f32);
            }
        }
    };
    renumber(&columns, &mut row);
    let none = Vec::new();
    for _ in 0..4 {
        for c in 1..column_count {
            sort_by_neighbours(&mut columns[c], &row, |slot| {
                slot_in.get(&slot).unwrap_or(&none)
            });
            renumber(&columns, &mut row);
        }
        for c in (0..column_count.saturating_sub(1)).rev() {
            sort_by_neighbours(&mut columns[c], &row, |slot| {
                slot_out.get(&slot).unwrap_or(&none)
            });
            renumber(&columns, &mut row);
        }
    }

    // 5. Coordinates: columns side by side, each centred on the tallest one.
    let size_of = |slot: Slot| match slot {
        Slot::Card(site) => sizes[site],
        Slot::Waypoint(_) => vec2(0.0, WAYPOINT_HEIGHT),
    };
    let height_of = |col: &[Slot]| -> f32 {
        col.iter().map(|&slot| size_of(slot).y).sum::<f32>()
            + ROW_GAP * col.len().saturating_sub(1) as f32
    };
    let tallest = columns.iter().map(|col| height_of(col)).fold(0.0, f32::max);

    let mut position: HashMap<Slot, Pos2> = HashMap::new();
    let mut left = 0.0;
    for col in &columns {
        let width = col
            .iter()
            .map(|&slot| size_of(slot).x)
            .fold(0.0, f32::max)
            .max(40.0);
        let mut y = (tallest - height_of(col)) / 2.0;
        for &slot in col {
            let size = size_of(slot);
            position.insert(slot, pos2(left + width / 2.0, y + size.y / 2.0));
            y += size.y + ROW_GAP;
        }
        left += width + COLUMN_GAP;
    }

    PlacedPart {
        centers: members
            .iter()
            .map(|&site| (site, position[&Slot::Card(site)]))
            .collect(),
        routes: long_edges
            .into_iter()
            .map(|(edge, waypoints)| (edge, waypoints.iter().map(|w| position[w]).collect()))
            .collect(),
        height: tallest,
    }
}

/// Sorts a column by the average row of each slot's neighbours; a slot without neighbours keeps
/// its row.
fn sort_by_neighbours<'a>(
    col: &mut [Slot],
    row: &HashMap<Slot, f32>,
    neighbours: impl Fn(Slot) -> &'a Vec<Slot>,
) {
    let mut keyed: Vec<(f32, Slot)> = col
        .iter()
        .map(|&slot| {
            let around = neighbours(slot);
            let key = if around.is_empty() {
                row[&slot]
            } else {
                around.iter().map(|other| row[other]).sum::<f32>() / around.len() as f32
            };
            (key, slot)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (place, (_, slot)) in col.iter_mut().zip(keyed) {
        *place = slot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    fn cards(n: usize) -> Vec<Vec2> {
        vec![vec2(160.0, 60.0); n]
    }

    #[test]
    fn a_chain_goes_left_to_right() {
        let centers = arrange(&cards(3), &[(0, 1), (1, 2)]).centers;

        assert!(centers[0].x < centers[1].x && centers[1].x < centers[2].x);
        assert_eq!(centers[0].y, centers[2].y);
    }

    #[test]
    fn an_edge_into_a_card_puts_it_after_its_furthest_sender() {
        // Start sends Hash and Save; Hash sends Save too, so Save is two columns in.
        let centers = arrange(&cards(3), &[(0, 1), (0, 2), (1, 2)]).centers;

        assert!(centers[2].x > centers[1].x);
    }

    #[test]
    fn an_edge_that_skips_a_column_is_routed_beside_the_card_there() {
        // Start sends Hash and Save; Hash sends Save, so Start → Save skips Hash's column.
        let sizes = cards(3);
        let placed = arrange(&sizes, &[(0, 1), (0, 2), (1, 2)]);
        let route = &placed.routes[&(0, 2)];

        assert_eq!(route.len(), 1);
        let hash = Rect::from_center_size(placed.centers[1], sizes[1]);
        assert!(
            !hash.contains(route[0]),
            "the waypoint is under Hash's card"
        );
        assert!((route[0].x - placed.centers[1].x).abs() < 1e-3);
    }

    #[test]
    fn loops_and_self_edges_still_place_every_card() {
        // TaskD sends itself, and TaskE and TaskD send each other.
        let centers = arrange(&cards(3), &[(0, 1), (1, 1), (1, 2), (2, 1)]).centers;

        assert!(centers.iter().all(|c| c.x.is_finite() && c.y.is_finite()));
        assert!(centers[1].x > centers[0].x);
    }

    #[test]
    fn a_part_made_only_of_a_loop_is_placed() {
        let centers = arrange(&cards(2), &[(0, 1), (1, 0)]).centers;

        assert_ne!(centers[0], centers[1]);
    }

    #[test]
    fn cards_in_a_column_never_overlap() {
        // One start sending five tasks: all five share a column.
        let edges: Vec<(usize, usize)> = (1..6).map(|to| (0, to)).collect();
        let sizes = cards(6);
        let centers = arrange(&sizes, &edges).centers;

        let mut ys: Vec<f32> = (1..6).map(|site| centers[site].y).collect();
        ys.sort_by(f32::total_cmp);
        for pair in ys.windows(2) {
            assert!(pair[1] - pair[0] >= sizes[1].y + ROW_GAP - 0.01);
        }
    }

    #[test]
    fn separate_parts_get_separate_bands() {
        let centers = arrange(&cards(4), &[(0, 1), (2, 3)]).centers;

        assert!(centers[2].y > centers[0].y + 60.0);
        assert_eq!(centers[0].x, centers[2].x);
    }
}
