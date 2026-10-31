//! Circular longitude arithmetic behind [`GeoBounds`](super::GeoBounds).

/// Reads longitude 180 as -180, the one meridian's single circular position.
pub(super) fn same_meridian(lon: f64) -> f64 {
    if lon == 180.0 { -180.0 } else { lon }
}

/// Writes an arc's edges so an east edge on the antimeridian reads `180`, unless
/// the arc is that meridian alone.
pub(super) fn canonical_edges(west: f64, east: f64) -> (f64, f64) {
    if east == -180.0 && west != -180.0 {
        (west, 180.0)
    } else {
        (west, east)
    }
}

/// The west and east edges of the smallest arc through `lons`, each already read
/// by [`same_meridian`]: the complement of the widest gap between neighbours.
/// With no longitudes (only poles) it is the prime meridian.
pub(super) fn smallest_arc(mut lons: Vec<f64>) -> (f64, f64) {
    lons.sort_by(f64::total_cmp);
    let (Some(&first), Some(&last)) = (lons.first(), lons.last()) else {
        return (0.0, 0.0);
    };
    // The wrap-around gap, from the last longitude round to the first, leaves
    // the plain `first..=last` arc.
    let (mut west, mut east) = (first, last);
    let mut widest = first + 360.0 - last;
    for pair in lons.windows(2) {
        let gap = pair[1] - pair[0];
        if gap > widest {
            widest = gap;
            (west, east) = (pair[1], pair[0]);
        }
    }
    canonical_edges(west, east)
}

/// An arc of longitudes running east from `west` to `east`.
#[derive(Debug, Clone, Copy)]
pub(super) struct LonArc {
    pub(super) west: f64,
    pub(super) east: f64,
}

impl LonArc {
    const FULL: Self = Self {
        west: -180.0,
        east: 180.0,
    };

    pub(super) fn meridian(lon: f64) -> Self {
        Self {
            west: lon,
            east: lon,
        }
    }

    fn is_full(self) -> bool {
        self.west == -180.0 && self.east == 180.0
    }

    /// The degrees east from `west` to `lon`, in `[0, 360)`.
    pub(super) fn offset(self, lon: f64) -> f64 {
        let delta = same_meridian(lon) - same_meridian(self.west);
        if delta < 0.0 { delta + 360.0 } else { delta }
    }

    pub(super) fn span(self) -> f64 {
        if self.is_full() {
            360.0
        } else {
            self.offset(self.east)
        }
    }

    /// Exact comparison against the edges, so an edge is always inside its own
    /// arc: no offset arithmetic, whose rounding could put it just outside.
    pub(super) fn contains(self, lon: f64) -> bool {
        let one = |lon: f64| {
            if self.west <= self.east {
                self.west <= lon && lon <= self.east
            } else {
                lon >= self.west || lon <= self.east
            }
        };
        one(lon) || (lon.abs() == 180.0 && one(-lon))
    }

    fn holds(self, inner: Self) -> bool {
        if self.is_full() {
            return true;
        }
        if inner.is_full() {
            return false;
        }
        let (start, end) = (self.offset(inner.west), self.offset(inner.east));
        start <= end && end <= self.span()
    }

    /// The shortest arc holding both: the one that already holds the other, else
    /// the west edge of one and the east edge of the other, unless together they
    /// circle the globe.
    pub(super) fn union(self, other: Self) -> Self {
        match (self.holds(other), other.holds(self)) {
            // One arc, written two ways around the antimeridian: pick one spelling.
            (true, true) if (other.west, other.east) < (self.west, self.east) => return other,
            (true, _) => return self,
            (false, true) => return other,
            (false, false) => {}
        }
        let one = Self {
            west: self.west,
            east: other.east,
        };
        let two = Self {
            west: other.west,
            east: self.east,
        };
        let holds_both = |arc: Self| arc.holds(self) && arc.holds(other);
        let chosen = match (holds_both(one), holds_both(two)) {
            (true, true) => {
                let (a, b) = (one.span(), two.span());
                if a < b || a == b && one.west <= two.west {
                    one
                } else {
                    two
                }
            }
            (true, false) => one,
            (false, true) => two,
            (false, false) => return Self::FULL,
        };
        let (west, east) = canonical_edges(chosen.west, chosen.east);
        Self { west, east }
    }
}
