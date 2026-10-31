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
impl KdTree {
    /// Whether frontier entries compare equal exactly when their chords match.
    pub(super) fn pending_eq_holds() -> bool {
        let pending = |chord_sq| Pending {
            chord_sq,
            item: Item::Node(0),
        };
        pending(1.0) == pending(1.0) && pending(1.0) != pending(2.0)
    }

    /// The squared distance from `query` to the box around every point.
    pub(super) fn root_box_chord_sq(&self, query: Vector) -> f64 {
        box_chord_sq(&self.nodes[0], query)
    }
}
