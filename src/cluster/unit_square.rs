//! The Web Mercator unit square `[0, 1] × [0, 1]`, north at `y = 0`: the one copy
//! of the projection that cluster pins, map tiles and map views share, so the pins
//! the index places and the views a map shows cannot drift apart. `WebMercator`'s
//! meters come from its own closed form instead.

/// `lon` as a fraction of the world's width, west edge 0, east edge 1.
pub(crate) fn unit_x(lon: f64) -> f64 {
    (lon + 180.0) / 360.0
}

/// `lat` as a fraction of the world's height, top edge 0, bottom edge 1, clamped to
/// the square, so a pole lands on its edge.
pub(crate) fn unit_y(lat: f64) -> f64 {
    let sin = lat.to_radians().sin();
    let y = 0.5 - ((1.0 + sin) / (1.0 - sin)).ln() / (4.0 * std::f64::consts::PI);
    y.clamp(0.0, 1.0)
}

/// The longitude at the unit-square column `x`, unwrapped: `x = 1` is the east
/// edge, 180, not the west edge it shares a meridian with. For a box's edges.
#[cfg(feature = "map")]
pub(crate) fn edge_lon_at(x: f64) -> f64 {
    x * 360.0 - 180.0
}

/// The longitude at the unit-square column `x`, wrapped into `[-180, 180)`.
#[cfg(feature = "map")]
pub(crate) fn lon_at(x: f64) -> f64 {
    edge_lon_at(x.rem_euclid(1.0))
}

/// The latitude at the unit-square row `y`, which lies in `[0, 1]`.
#[cfg(feature = "map")]
pub(crate) fn lat_at(y: f64) -> f64 {
    (std::f64::consts::PI * (1.0 - 2.0 * y))
        .sinh()
        .atan()
        .to_degrees()
}
