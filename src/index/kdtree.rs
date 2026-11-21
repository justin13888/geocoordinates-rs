//! A static k-d tree over unit vectors: the candidate prefilter behind
//! [`SpatialIndex`](super::SpatialIndex). It knows nothing of geodesics. It
//! reports squared chord lengths, and the caller turns them into distance bounds.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Points per leaf: small enough to prune well, large enough that the tree
/// overhead does not dominate a query.
const LEAF_SIZE: usize = 16;

type Vector = [f64; 3];

/// One tree node over `points[start..end]`, with the tight box around them.
#[derive(Debug, Clone)]
struct Node {
    low: Vector,
    high: Vector,
    start: usize,
    end: usize,
    /// The two child nodes, or `None` for a leaf.
    children: Option<(usize, usize)>,
}

/// The tree: nodes in a flat vector, points in tree order with their ids.
#[derive(Debug, Clone)]
pub(super) struct KdTree {
    nodes: Vec<Node>,
    points: Vec<(Vector, usize)>,
}

fn chord_sq(a: Vector, b: Vector) -> f64 {
    (0..3).map(|axis| (a[axis] - b[axis]).powi(2)).sum()
}

/// The squared distance from `query` to the nearest point of the node's box.
fn box_chord_sq(node: &Node, query: Vector) -> f64 {
    (0..3)
        .map(|axis| {
            (node.low[axis] - query[axis])
                .max(query[axis] - node.high[axis])
                .max(0.0)
                .powi(2)
        })
        .sum()
}

impl KdTree {
    /// Builds the tree; each vector's id is its position in `vectors`.
    pub(super) fn build(vectors: impl IntoIterator<Item = Vector>) -> Self {
        let mut tree = Self {
            nodes: Vec::new(),
            points: vectors
                .into_iter()
                .enumerate()
                .map(|(id, vector)| (vector, id))
                .collect(),
        };
        if !tree.points.is_empty() {
            tree.build_node(0, tree.points.len());
        }
        tree
    }

    /// Adds the node over `points[start..end]` and its subtree, returning its
    /// position. Each split halves the range, so recursion is `log2(n)` deep.
    fn build_node(&mut self, start: usize, end: usize) -> usize {
        let (low, high) = self.points[start..end].iter().fold(
            ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
            |(mut low, mut high), (vector, _)| {
                for axis in 0..3 {
                    low[axis] = low[axis].min(vector[axis]);
                    high[axis] = high[axis].max(vector[axis]);
                }
                (low, high)
            },
        );
        let position = self.nodes.len();
        self.nodes.push(Node {
            low,
            high,
            start,
            end,
            children: None,
        });
        if end - start > LEAF_SIZE {
            let axis = (0..3)
                .max_by(|&a, &b| (high[a] - low[a]).total_cmp(&(high[b] - low[b])))
                .unwrap_or(0);
            let middle = (end - start) / 2;
            // Ties break on the id, so a build is deterministic.
            self.points[start..end].select_nth_unstable_by(middle, |a, b| {
                a.0[axis].total_cmp(&b.0[axis]).then(a.1.cmp(&b.1))
            });
            let left = self.build_node(start, start + middle);
            let right = self.build_node(start + middle, end);
            self.nodes[position].children = Some((left, right));
        }
        position
    }

    /// Calls `visit` with the id of every point within squared chord `limit_sq`
    /// of `query`, in no particular order.
    pub(super) fn for_each_within(
        &self,
        query: Vector,
        limit_sq: f64,
        mut visit: impl FnMut(usize),
    ) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![0];
        while let Some(position) = stack.pop() {
            let node = &self.nodes[position];
            if box_chord_sq(node, query) > limit_sq {
                continue;
            }
            match node.children {
                Some((left, right)) => stack.extend([left, right]),
                None => {
                    for &(vector, id) in &self.points[node.start..node.end] {
                        if chord_sq(vector, query) <= limit_sq {
                            visit(id);
                        }
                    }
                }
            }
        }
    }

    /// Every point as `(squared chord, id)`, lazily, by rising squared chord.
    pub(super) fn by_chord(&self, query: Vector) -> ByChord<'_> {
        let mut frontier = BinaryHeap::new();
        if let Some(root) = self.nodes.first() {
            frontier.push(Pending {
                chord_sq: box_chord_sq(root, query),
                item: Item::Node(0),
            });
        }
        ByChord {
            tree: self,
            query,
            frontier,
        }
    }
}

/// The iterator of [`KdTree::by_chord`]: a best-first walk of the tree.
#[derive(Debug)]
pub(super) struct ByChord<'a> {
    tree: &'a KdTree,
    query: Vector,
    frontier: BinaryHeap<Pending>,
}

impl Iterator for ByChord<'_> {
    type Item = (f64, usize);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(pending) = self.frontier.pop() {
            let position = match pending.item {
                Item::Point(id) => return Some((pending.chord_sq, id)),
                Item::Node(position) => position,
            };
            let node = &self.tree.nodes[position];
            match node.children {
                Some((left, right)) => {
                    for child in [left, right] {
                        self.frontier.push(Pending {
                            chord_sq: box_chord_sq(&self.tree.nodes[child], self.query),
                            item: Item::Node(child),
                        });
                    }
                }
                None => {
                    for &(vector, id) in &self.tree.points[node.start..node.end] {
                        self.frontier.push(Pending {
                            chord_sq: chord_sq(vector, self.query),
                            item: Item::Point(id),
                        });
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
enum Item {
    Node(usize),
    Point(usize),
}

/// A frontier entry, ordered so the max-heap pops the smallest chord first.
#[derive(Debug, Clone, Copy)]
struct Pending {
    chord_sq: f64,
    item: Item,
}

impl PartialEq for Pending {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Pending {}

impl PartialOrd for Pending {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pending {
    fn cmp(&self, other: &Self) -> Ordering {
        other.chord_sq.total_cmp(&self.chord_sq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unit vector of a latitude and longitude on the sphere, in degrees.
    fn unit(lat: f64, lon: f64) -> Vector {
        let (lat, lon) = (lat.to_radians(), lon.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    }

    #[test]
    fn frontier_entries_compare_equal_only_when_their_chords_match() {
        let pending = |chord_sq| Pending {
            chord_sq,
            item: Item::Node(0),
        };
        assert!(pending(1.0) == pending(1.0));
        assert!(pending(1.0) != pending(2.0));
    }

    #[test]
    fn by_chord_yields_every_point_by_rising_chord() {
        let tree =
            KdTree::build((0..100).map(|i| unit(f64::from(i) - 50.0, f64::from(i) * 3.0 - 150.0)));
        let query = unit(10.0, 10.0);
        // One past the expected count: a walk that overruns fails here, not by hanging.
        let walk: Vec<(f64, usize)> = tree.by_chord(query).take(101).collect();
        assert_eq!(walk.len(), 100);
        assert!(walk.windows(2).all(|pair| pair[0].0 <= pair[1].0));
        let mut ids: Vec<usize> = walk.iter().map(|&(_, id)| id).collect();
        ids.sort_unstable();
        assert_eq!(ids, (0..100).collect::<Vec<_>>());
        assert_eq!(KdTree::build([]).by_chord([1.0, 0.0, 0.0]).count(), 0);
        let mut none = 0;
        KdTree::build([]).for_each_within([1.0, 0.0, 0.0], 4.0, |_| none += 1);
        assert_eq!(none, 0);
    }

    #[test]
    fn by_chord_reports_the_squared_chord_to_each_point() {
        let tree = KdTree::build([[1.0, 2.0, 3.0], [-1.0, 0.0, 2.0]]);
        let walk: Vec<(f64, usize)> = tree.by_chord([1.0, 0.0, 0.0]).take(3).collect();
        assert_eq!(walk, [(8.0, 1), (13.0, 0)]);
    }

    #[test]
    fn for_each_within_keeps_only_points_inside_the_limit() {
        let tree = KdTree::build([[1.0, 2.0, 3.0], [-1.0, 0.0, 2.0]]);
        let mut found = Vec::new();
        tree.for_each_within([1.0, 0.0, 0.0], 8.0, |id| found.push(id));
        assert_eq!(found, [1]);
    }

    #[test]
    fn the_box_distance_sums_the_gap_along_each_axis() {
        let tree = KdTree::build([[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]);
        // Past the high side on x, inside on y, past the low side on z.
        assert_eq!(box_chord_sq(&tree.nodes[0], [3.0, 0.5, -1.0]), 5.0);
        assert_eq!(box_chord_sq(&tree.nodes[0], [0.5, 0.5, 0.5]), 0.0);
    }
}
