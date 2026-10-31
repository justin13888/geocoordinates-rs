//! The per-zoom cluster forest behind [`ClusterIndex`](super::ClusterIndex).
//!
//! Level `z` (`0..=22`) clusters the nodes of level `z + 1`, and level 23 holds the
//! points. A node's id is a point id: that of the seed it grew from, which also
//! names its seed's node on the level below, down to the point itself.
//!
//! ## The rule that makes the result canonical
//!
//! Nodes are ordered by their point's latitude, longitude and id. At level `z`, taken
//! in that order, a child is a *seed* when no earlier seed lies within the level's
//! radius of it; otherwise it joins the earliest seed that does. The seeds are the
//! lexicographically first maximal independent set, which is a function of the
//! children and their order alone. That is what makes the result independent of the
//! order points arrived in.
//!
//! It is also why an update is local. A child's fate reads only earlier neighbors, so
//! a change at one child can reach only later children within the radius of a child
//! whose seed status changed. [`Forest::apply`] walks those in order, each once.
//! Aggregates sum their children in order, so the bits match a rebuild's exactly.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use super::super::centroid::{ROUND_OFF_M, angle_between, from_sum, normal};
use crate::coord::{Coordinate, Crs};
use crate::index::GeoBounds;

/// The deepest cluster level.
pub(super) const DEEPEST: usize = 22;
/// The level holding the points themselves.
pub(super) const LEAF_LEVEL: usize = DEEPEST + 1;
/// A grid never has more columns than this, however small the radius.
const MAX_CELLS: f64 = 2_147_483_648.0;
/// Marks a node not yet given a parent.
const NO_PARENT: u32 = u32::MAX;

/// What a node summarizes. A node with one child shares that child's, which is what
/// keeps levels of unmerged points cheap.
#[derive(Debug, PartialEq)]
pub(super) struct Aggregate {
    sum: [f64; 3],
    pub(super) count: u64,
    pub(super) centroid: Coordinate,
    x: f64,
    y: f64,
    pub(super) max_error_m: f64,
    pub(super) bounds: GeoBounds,
}

impl Aggregate {
    fn point(point: Coordinate) -> Self {
        let (x, y) = world_xy(point);
        Self {
            sum: normal(point),
            count: 1,
            centroid: point,
            x,
            y,
            max_error_m: 0.0,
            bounds: GeoBounds::from_points([point]).expect("one point on one CRS"),
        }
    }

    /// The summary of `children`, which are in canonical order.
    fn merged(children: &[Arc<Self>], crs: Crs, normal_radius_m: f64) -> Self {
        let mut sum = [0.0; 3];
        let mut count = 0;
        let mut bounds = children[0].bounds;
        for child in children {
            sum = [
                sum[0] + child.sum[0],
                sum[1] + child.sum[1],
                sum[2] + child.sum[2],
            ];
            count += child.count;
            bounds = bounds.union(&child.bounds).expect("one CRS per index");
        }
        // Children that cancel have no mean direction. The bound below stays true
        // for any point, so the seed's own centroid stands in.
        let centroid = from_sum(sum, count, crs).unwrap_or(children[0].centroid);
        let center = normal(centroid);
        let reach = children
            .iter()
            .map(|child| {
                let apart = angle_between(center, normal(child.centroid)) * normal_radius_m;
                apart + child.max_error_m
            })
            .fold(0.0, f64::max);
        let (x, y) = world_xy(centroid);
        Self {
            sum,
            count,
            centroid,
            x,
            y,
            max_error_m: reach + ROUND_OFF_M,
            bounds,
        }
    }
}

/// `point` on the Web Mercator square `[0, 1] × [0, 1]`, north at `y = 0`. Latitudes
/// past about ±85.05° clamp to the square's edge. `x = 0` and `x = 1` are one meridian:
/// distances wrap, and the grid's first and last columns are neighbors.
fn world_xy(point: Coordinate) -> (f64, f64) {
    (world_x(point.lon()), world_y(point.lat()))
}

pub(super) fn world_x(lon: f64) -> f64 {
    (lon + 180.0) / 360.0
}

pub(super) fn world_y(lat: f64) -> f64 {
    let sin = lat.to_radians().sin();
    let y = 0.5 - ((1.0 + sin) / (1.0 - sin)).ln() / (4.0 * std::f64::consts::PI);
    y.clamp(0.0, 1.0)
}

/// One cluster on one level.
#[derive(Debug)]
pub(super) struct Node {
    pub(super) aggregate: Arc<Aggregate>,
    parent: u32,
    /// Ids on the next level down, in canonical order. Empty means just the node's
    /// own id, so a node that merged nothing allocates nothing.
    children: Vec<u32>,
}

impl Node {
    pub(super) fn children(&self, id: u32) -> Vec<u32> {
        if self.children.is_empty() {
            vec![id]
        } else {
            self.children.clone()
        }
    }
}

/// A node's place in the canonical order.
#[derive(Debug, Clone, Copy)]
struct Key {
    lat: f64,
    lon: f64,
    id: u32,
}

impl Key {
    fn order(&self, other: &Self) -> Ordering {
        self.lat
            .total_cmp(&other.lat)
            .then(self.lon.total_cmp(&other.lon))
            .then(self.id.cmp(&other.id))
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.order(other) == Ordering::Equal
    }
}

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order(other)
    }
}

/// Every level's nodes, and a grid over them.
#[derive(Debug)]
pub(super) struct Forest {
    crs: Crs,
    normal_radius_m: f64,
    /// The cluster radius of each level `0..=22`, as a fraction of the world's width.
    radii: Vec<f64>,
    /// Each point's latitude and longitude by id, kept after removal so a removed
    /// child can still be ordered while the update walks past it.
    keys: Vec<(f64, f64)>,
    /// Nodes by level, then by id.
    levels: Vec<Vec<Option<Node>>>,
    /// `(column << 32 | row, id)` by level. A level's grid is sized by the radius
    /// that clusters it, which is the radius of the level above.
    grids: Vec<BTreeSet<(u64, u32)>>,
}

/// The columns (and rows) of the grid over level `level`.
fn cells_for(radii: &[f64], level: usize) -> u64 {
    // Level 0 has no level above; a grid twice as coarse as its own radius serves.
    let radius = if level == 0 {
        radii[0] * 2.0
    } else {
        radii[level - 1]
    };
    (1.0 / radius).floor().clamp(1.0, MAX_CELLS) as u64
}

fn cell_of(fraction: f64, cells: u64) -> u64 {
    ((fraction * cells as f64) as u64).min(cells - 1)
}

impl Forest {
    /// The forest over `points`, whose ids are their positions.
    pub(super) fn build(
        points: &[Coordinate],
        crs: Crs,
        normal_radius_m: f64,
        radii: Vec<f64>,
    ) -> Self {
        let mut forest = Self {
            crs,
            normal_radius_m,
            radii,
            keys: points.iter().map(|p| (p.lat(), p.lon())).collect(),
            levels: Vec::with_capacity(LEAF_LEVEL + 1),
            grids: vec![BTreeSet::new(); LEAF_LEVEL + 1],
        };
        forest.levels.resize_with(LEAF_LEVEL + 1, Vec::new);

        let mut leaf_nodes = Vec::with_capacity(points.len());
        for point in points {
            leaf_nodes.push(Some(Node {
                aggregate: Arc::new(Aggregate::point(*point)),
                parent: NO_PARENT,
                children: Vec::new(),
            }));
        }
        forest.levels[LEAF_LEVEL] = leaf_nodes;

        let mut current: Vec<u32> = (0..points.len() as u32).collect();
        current.sort_unstable_by_key(|id| forest.key(*id));
        for level in (0..=DEEPEST).rev() {
            forest.fill_grid(level + 1, &current);
            current = forest.build_level(level, &current);
        }
        forest.fill_grid(0, &current);
        forest
    }

    fn fill_grid(&mut self, level: usize, ids: &[u32]) {
        let grid = ids
            .iter()
            .map(|id| {
                let aggregate = &self.node(level, *id).aggregate;
                (self.cell(level, aggregate.x, aggregate.y), *id)
            })
            .collect();
        self.grids[level] = grid;
    }

    /// Clusters `children`, the nodes of `level + 1` in canonical order, returning
    /// the seeds in that order.
    fn build_level(&mut self, level: usize, children: &[u32]) -> Vec<u32> {
        let mut nodes: Vec<Option<Node>> = Vec::new();
        nodes.resize_with(self.keys.len(), || None);
        let mut seeds = Vec::new();
        let mut near = Vec::new();
        for &seed in children {
            if self.node(level + 1, seed).parent != NO_PARENT {
                continue;
            }
            near.clear();
            let at = Arc::clone(&self.node(level + 1, seed).aggregate);
            self.for_each_near(level, at.x, at.y, |id| near.push(id));
            near.retain(|id| self.node(level + 1, *id).parent == NO_PARENT);
            near.sort_unstable_by_key(|id| self.key(*id));
            for id in &near {
                self.node_mut(level + 1, *id).parent = seed;
            }
            let children = if near.len() == 1 {
                Vec::new()
            } else {
                near.clone()
            };
            nodes[seed as usize] = Some(Node {
                aggregate: self.aggregate_of(level, seed, &children),
                parent: NO_PARENT,
                children,
            });
            seeds.push(seed);
        }
        self.levels[level] = nodes;
        seeds
    }

    /// The summary of a node at `level` from its children on the level below.
    fn aggregate_of(&self, level: usize, id: u32, children: &[u32]) -> Arc<Aggregate> {
        if children.len() <= 1 {
            let only = children.first().copied().unwrap_or(id);
            return Arc::clone(&self.node(level + 1, only).aggregate);
        }
        let parts: Vec<Arc<Aggregate>> = children
            .iter()
            .map(|child| Arc::clone(&self.node(level + 1, *child).aggregate))
            .collect();
        Arc::new(Aggregate::merged(&parts, self.crs, self.normal_radius_m))
    }

    fn key(&self, id: u32) -> Key {
        let (lat, lon) = self.keys[id as usize];
        Key { lat, lon, id }
    }

    /// The node `id` on `level`, which the caller knows is there.
    pub(super) fn node(&self, level: usize, id: u32) -> &Node {
        self.levels[level][id as usize]
            .as_ref()
            .expect("a live node")
    }

    fn node_mut(&mut self, level: usize, id: u32) -> &mut Node {
        self.levels[level][id as usize]
            .as_mut()
            .expect("a live node")
    }

    pub(super) fn get(&self, level: usize, id: u32) -> Option<&Node> {
        self.levels[level].get(id as usize)?.as_ref()
    }

    fn cell(&self, level: usize, x: f64, y: f64) -> u64 {
        let cells = cells_for(&self.radii, level);
        cell_of(x, cells) << 32 | cell_of(y, cells)
    }

    /// Calls `visit` with each node of `level + 1` within `level`'s radius of `(x, y)`
    /// on the world square, measuring longitude the short way around.
    fn for_each_near(&self, level: usize, x: f64, y: f64, mut visit: impl FnMut(u32)) {
        let cells = cells_for(&self.radii, level + 1);
        let (column, row) = (cell_of(x, cells), cell_of(y, cells));
        let columns = if cells <= 3 {
            (0..cells).collect::<Vec<_>>()
        } else {
            vec![(column + cells - 1) % cells, column, (column + 1) % cells]
        };
        let (low, high) = (row.saturating_sub(1), (row + 1).min(cells - 1));
        let radius_sq = self.radii[level] * self.radii[level];
        for column in columns {
            let range = (column << 32 | low, 0)..=(column << 32 | high, u32::MAX);
            for (_, id) in self.grids[level + 1].range(range) {
                let at = &self.node(level + 1, *id).aggregate;
                let dx = (at.x - x).abs();
                let (dx, dy) = (dx.min(1.0 - dx), at.y - y);
                if dx * dx + dy * dy <= radius_sq {
                    visit(*id);
                }
            }
        }
    }

    /// Sets the CRS later merges are computed on.
    pub(super) fn set_model(&mut self, crs: Crs, normal_radius_m: f64) {
        self.crs = crs;
        self.normal_radius_m = normal_radius_m;
    }

    /// Adds a point under the next id, which is `keys.len()`.
    pub(super) fn insert(&mut self, point: Coordinate) -> u32 {
        let id = self.keys.len() as u32;
        self.keys.push((point.lat(), point.lon()));
        for level in &mut self.levels {
            level.push(None);
        }
        let aggregate = Arc::new(Aggregate::point(point));
        let cell = self.cell(LEAF_LEVEL, aggregate.x, aggregate.y);
        self.grids[LEAF_LEVEL].insert((cell, id));
        self.levels[LEAF_LEVEL][id as usize] = Some(Node {
            aggregate,
            parent: NO_PARENT,
            children: Vec::new(),
        });
        self.apply(vec![id], HashMap::new());
        id
    }

    /// Removes the point `id`, which the caller knows is live.
    pub(super) fn remove(&mut self, id: u32) {
        let node = self.levels[LEAF_LEVEL][id as usize]
            .take()
            .expect("a live point");
        let cell = self.cell(LEAF_LEVEL, node.aggregate.x, node.aggregate.y);
        self.grids[LEAF_LEVEL].remove(&(cell, id));
        self.apply(vec![id], HashMap::from([(id, node.parent)]));
    }

    /// Carries changes upward. `changed` are nodes of the level below the one being
    /// redone that were created, removed or given a new aggregate, and `removed`
    /// holds the old parent of each that was removed.
    fn apply(&mut self, mut changed: Vec<u32>, mut removed: HashMap<u32, u32>) {
        for level in (0..=DEEPEST).rev() {
            if changed.is_empty() {
                return;
            }
            (changed, removed) = self.update_level(level, &changed, &removed);
        }
    }

    /// Redoes `level` for the `changed` nodes of `level + 1`, returning the same two
    /// lists for the level above.
    fn update_level(
        &mut self,
        level: usize,
        changed: &[u32],
        removed: &HashMap<u32, u32>,
    ) -> (Vec<u32>, HashMap<u32, u32>) {
        let moved: HashSet<u32> = changed.iter().copied().collect();
        let mut work: BTreeSet<Key> = changed.iter().map(|id| self.key(*id)).collect();
        let mut dirty: BTreeSet<u32> = BTreeSet::new();
        let mut created: HashSet<u32> = HashSet::new();
        let mut next_changed = Vec::new();
        let mut next_removed = HashMap::new();

        // Pushes only ever go to later keys, so each is taken once. The bound keeps
        // this loop finite whatever the pushes do.
        for _ in 0..self.keys.len() {
            let Some(key) = work.pop_first() else {
                break;
            };
            let id = key.id;
            let child = self.levels[level + 1][id as usize].as_ref();
            let old_parent = match child {
                Some(node) => node.parent,
                None => removed[&id],
            };
            let new_parent = child.map(|node| self.seed_for(level, key, &node.aggregate));
            let was_seed = self.levels[level][id as usize].is_some();
            let is_moved = moved.contains(&id);
            let stays_seed = new_parent == Some(id);

            if was_seed && (!stays_seed || is_moved) {
                for other in self.node(level, id).children(id) {
                    if other != id {
                        work.insert(self.key(other));
                    }
                }
            }
            if stays_seed && (!was_seed || is_moved) {
                let at = Arc::clone(&self.node(level + 1, id).aggregate);
                let keys = &mut work;
                let forest = &*self;
                forest.for_each_near(level, at.x, at.y, |other| {
                    let other_key = forest.key(other);
                    if other_key > key {
                        keys.insert(other_key);
                    }
                });
            }

            if Some(old_parent) != new_parent && old_parent != NO_PARENT {
                self.detach(level, old_parent, id);
                dirty.insert(old_parent);
            }
            if was_seed && !stays_seed {
                let node = self.levels[level][id as usize].take().expect("was a seed");
                let cell = self.cell(level, node.aggregate.x, node.aggregate.y);
                self.grids[level].remove(&(cell, id));
                dirty.remove(&id);
                next_removed.insert(id, node.parent);
                next_changed.push(id);
            }
            if stays_seed && !was_seed {
                self.levels[level][id as usize] = Some(Node {
                    aggregate: Arc::clone(&self.node(level + 1, id).aggregate),
                    parent: NO_PARENT,
                    children: Vec::new(),
                });
                created.insert(id);
                next_changed.push(id);
            }
            if let Some(parent) = new_parent {
                if old_parent != parent {
                    self.attach(level, parent, id);
                    self.node_mut(level + 1, id).parent = parent;
                }
                if is_moved || old_parent != parent {
                    dirty.insert(parent);
                }
            }
        }

        for id in dirty {
            let is_new = created.contains(&id);
            let Some(node) = self.levels[level][id as usize].as_ref() else {
                continue;
            };
            let old = Arc::clone(&node.aggregate);
            let aggregate = self.aggregate_of(level, id, &node.children);
            if !is_new {
                if *aggregate == *old {
                    continue;
                }
                let cell = self.cell(level, old.x, old.y);
                self.grids[level].remove(&(cell, id));
                next_changed.push(id);
            }
            let cell = self.cell(level, aggregate.x, aggregate.y);
            self.grids[level].insert((cell, id));
            self.node_mut(level, id).aggregate = aggregate;
        }
        (next_changed, next_removed)
    }

    /// The seed `key`'s node joins at `level`: the earliest seed within the radius,
    /// which is the node itself when no earlier one is.
    fn seed_for(&self, level: usize, key: Key, at: &Aggregate) -> u32 {
        let mut best = key;
        self.for_each_near(level, at.x, at.y, |other| {
            let other_key = self.key(other);
            if other_key < best && self.levels[level][other as usize].is_some() {
                best = other_key;
            }
        });
        best.id
    }

    fn detach(&mut self, level: usize, parent: u32, child: u32) {
        let Some(node) = self.levels[level][parent as usize].as_mut() else {
            return; // the seed is already gone, and its children with it
        };
        node.children.retain(|id| *id != child);
        if node.children.len() == 1 {
            // Only the seed itself is left, which an empty list stands for.
            node.children = Vec::new();
        }
    }

    fn attach(&mut self, level: usize, parent: u32, child: u32) {
        let keys = &self.keys;
        let key_of = |id: u32| {
            let (lat, lon) = keys[id as usize];
            Key { lat, lon, id }
        };
        let node = self.levels[level][parent as usize]
            .as_mut()
            .expect("a live seed");
        if child == parent && node.children.is_empty() {
            return; // a node alone holds no list
        }
        if node.children.is_empty() {
            node.children.push(parent);
        }
        let at = node
            .children
            .binary_search_by_key(&key_of(child), |id| key_of(*id))
            .unwrap_or_else(|at| at);
        if node.children.get(at) != Some(&child) {
            node.children.insert(at, child);
        }
    }

    /// The ids of `level`'s nodes in the cells a box of longitude arc `[west, west +
    /// span]` degrees and world rows `[top, bottom]` touches, with a cell of slack
    /// around it for round-off; every node when that is the cheaper walk.
    pub(super) fn candidates(
        &self,
        level: usize,
        west: f64,
        span: f64,
        rows: Option<(f64, f64)>,
    ) -> Vec<u32> {
        let cells = cells_for(&self.radii, level);
        let grid = &self.grids[level];
        let columns = ((span / 360.0 * cells as f64).ceil() as u64)
            .saturating_add(3)
            .min(cells);
        let Some((top, bottom)) = rows.filter(|_| columns < grid.len() as u64) else {
            return grid.iter().map(|(_, id)| *id).collect();
        };
        let start = (cell_of(world_x(west), cells) + cells - 1) % cells;
        let low = cell_of(top, cells).saturating_sub(1);
        let high = (cell_of(bottom, cells) + 1).min(cells - 1);
        let mut found = Vec::new();
        for step in 0..columns {
            let column = (start + step) % cells;
            let range = (column << 32 | low, 0)..=(column << 32 | high, u32::MAX);
            found.extend(grid.range(range).map(|(_, id)| *id));
        }
        found
    }
}
