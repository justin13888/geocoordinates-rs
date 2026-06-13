//! Local tangent-plane frames: ENU, NED, and AER.
//!
//! These are defined relative to a reference origin, so they are expressed as
//! methods taking the origin rather than `From` impls. The math is **exact**
//! (a rotation of the ECEF difference vector), so results are bare types.

use super::ecef::Ecef;
use super::ellipsoid::Ellipsoid;
use crate::coord::Coordinate;
use crate::units::Length;

/// East-North-Up offset from a reference origin, in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Enu {
    /// East offset (meters).
    pub east: f64,
    /// North offset (meters).
    pub north: f64,
    /// Up offset (meters).
    pub up: f64,
}

/// North-East-Down offset from a reference origin, in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ned {
    /// North offset (meters).
    pub north: f64,
    /// East offset (meters).
    pub east: f64,
    /// Down offset (meters).
    pub down: f64,
}

/// Azimuth-Elevation-Range relative to a reference origin.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aer {
    /// Azimuth (degrees clockwise from north).
    pub azimuth_deg: f64,
    /// Elevation (degrees above the local horizontal).
    pub elevation_deg: f64,
    /// Slant range.
    pub range: Length,
}

impl Enu {
    /// The ENU offset of `target` relative to `origin` (exact, WGS-84).
    #[must_use]
    pub fn from_coordinate(target: Coordinate, origin: Coordinate) -> Self {
        let (sin_lat, cos_lat) = origin.lat.to_radians().sin_cos();
        let (sin_lon, cos_lon) = origin.lon.to_radians().sin_cos();
        let t = Ecef::from_coordinate(target, Ellipsoid::WGS84);
        let o = Ecef::from_coordinate(origin, Ellipsoid::WGS84);
        let (dx, dy, dz) = (t.x - o.x, t.y - o.y, t.z - o.z);
        Enu {
            east: -sin_lon * dx + cos_lon * dy,
            north: -sin_lat * cos_lon * dx - sin_lat * sin_lon * dy + cos_lat * dz,
            up: cos_lat * cos_lon * dx + cos_lat * sin_lon * dy + sin_lat * dz,
        }
    }

    /// Recover the absolute coordinate of this ENU offset about `origin`.
    #[must_use]
    pub fn to_coordinate(self, origin: Coordinate) -> Coordinate {
        let (sin_lat, cos_lat) = origin.lat.to_radians().sin_cos();
        let (sin_lon, cos_lon) = origin.lon.to_radians().sin_cos();
        // Rotate the ENU offset back to an ECEF difference (the transpose).
        let dx =
            -sin_lon * self.east - sin_lat * cos_lon * self.north + cos_lat * cos_lon * self.up;
        let dy = cos_lon * self.east - sin_lat * sin_lon * self.north + cos_lat * sin_lon * self.up;
        let dz = cos_lat * self.north + sin_lat * self.up;
        let o = Ecef::from_coordinate(origin, Ellipsoid::WGS84);
        Ecef::new(o.x + dx, o.y + dy, o.z + dz).to_coordinate(Ellipsoid::WGS84)
    }

    /// Convert to the NED convention (exact).
    #[must_use]
    pub fn to_ned(self) -> Ned {
        Ned {
            north: self.north,
            east: self.east,
            down: -self.up,
        }
    }

    /// Convert to azimuth/elevation/range (exact).
    #[must_use]
    pub fn to_aer(self) -> Aer {
        let range = (self.east * self.east + self.north * self.north + self.up * self.up).sqrt();
        let azimuth_deg = crate::angle::normalize_degrees(self.east.atan2(self.north).to_degrees());
        let elevation_deg = if range > 0.0 {
            (self.up / range).asin().to_degrees()
        } else {
            0.0
        };
        Aer {
            azimuth_deg,
            elevation_deg,
            range: Length::from_meters(range),
        }
    }
}

impl Ned {
    /// The NED offset of `target` relative to `origin` (exact).
    #[must_use]
    pub fn from_coordinate(target: Coordinate, origin: Coordinate) -> Self {
        Enu::from_coordinate(target, origin).to_ned()
    }

    /// Recover the absolute coordinate of this NED offset about `origin`.
    #[must_use]
    pub fn to_coordinate(self, origin: Coordinate) -> Coordinate {
        self.to_enu().to_coordinate(origin)
    }

    /// Convert to the ENU convention (exact).
    #[must_use]
    pub fn to_enu(self) -> Enu {
        Enu {
            east: self.east,
            north: self.north,
            up: -self.down,
        }
    }

    /// Convert to azimuth/elevation/range (exact).
    #[must_use]
    pub fn to_aer(self) -> Aer {
        self.to_enu().to_aer()
    }
}

impl Aer {
    /// The azimuth/elevation/range of `target` relative to `origin` (exact).
    #[must_use]
    pub fn from_coordinate(target: Coordinate, origin: Coordinate) -> Self {
        Enu::from_coordinate(target, origin).to_aer()
    }

    /// Recover the absolute coordinate of this AER offset about `origin`.
    #[must_use]
    pub fn to_coordinate(self, origin: Coordinate) -> Coordinate {
        self.to_enu().to_coordinate(origin)
    }

    /// Convert to the ENU convention (exact).
    #[must_use]
    pub fn to_enu(self) -> Enu {
        let az = self.azimuth_deg.to_radians();
        let el = self.elevation_deg.to_radians();
        let r = self.range.meters();
        Enu {
            east: r * el.cos() * az.sin(),
            north: r * el.cos() * az.cos(),
            up: r * el.sin(),
        }
    }

    /// Convert to the NED convention (exact).
    #[must_use]
    pub fn to_ned(self) -> Ned {
        self.to_enu().to_ned()
    }
}

// Frame-to-frame conversions are exact and origin-independent (pure rotation /
// repackaging), so they also implement `From`, per the conversion convention.
impl From<Enu> for Ned {
    fn from(enu: Enu) -> Ned {
        enu.to_ned()
    }
}
impl From<Ned> for Enu {
    fn from(ned: Ned) -> Enu {
        ned.to_enu()
    }
}
impl From<Enu> for Aer {
    fn from(enu: Enu) -> Aer {
        enu.to_aer()
    }
}
impl From<Aer> for Enu {
    fn from(aer: Aer) -> Enu {
        aer.to_enu()
    }
}
impl From<Ned> for Aer {
    fn from(ned: Ned) -> Aer {
        ned.to_aer()
    }
}
impl From<Aer> for Ned {
    fn from(aer: Aer) -> Ned {
        aer.to_ned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coord::Height;
    use crate::test_support::assert_close;

    #[test]
    fn enu_directions() {
        let origin = Coordinate::wgs84(40.0, -75.0);
        // A point due north of the origin has +north, ~0 east.
        let north = Enu::from_coordinate(Coordinate::wgs84(40.1, -75.0), origin);
        assert!(north.north > 10_000.0);
        assert_close(north.east, 0.0, 1e-6);
        // A point due east is predominantly east; the small north component is
        // the parallel's poleward curvature (~5 m over ~8.5 km).
        let east = Enu::from_coordinate(Coordinate::wgs84(40.0, -74.9), origin);
        assert!(east.east > 8_000.0);
        assert!(east.north.abs() < 50.0);
    }

    #[test]
    fn enu_round_trip() {
        let origin = Coordinate::wgs84(40.7128, -74.006);
        let target = Coordinate::wgs84(40.73, -74.0).with_height(Height::Ellipsoidal(120.0));
        let back = Enu::from_coordinate(target, origin).to_coordinate(origin);
        assert_close(back.lat, 40.73, 1e-9);
        assert_close(back.lon, -74.0, 1e-9);
        match back.height {
            Some(Height::Ellipsoidal(h)) => assert_close(h, 120.0, 1e-4),
            other => panic!("expected ellipsoidal height, got {other:?}"),
        }
    }

    #[test]
    fn enu_to_aer_reference() {
        let north = Enu {
            east: 0.0,
            north: 100.0,
            up: 0.0,
        }
        .to_aer();
        assert_close(north.azimuth_deg, 0.0, 1e-9);
        assert_close(north.elevation_deg, 0.0, 1e-9);
        assert_close(north.range.meters(), 100.0, 1e-9);

        let east = Enu {
            east: 100.0,
            north: 0.0,
            up: 0.0,
        }
        .to_aer();
        assert_close(east.azimuth_deg, 90.0, 1e-9);

        let up = Enu {
            east: 0.0,
            north: 0.0,
            up: 100.0,
        }
        .to_aer();
        assert_close(up.elevation_deg, 90.0, 1e-9);
        assert_close(up.range.meters(), 100.0, 1e-9);
    }

    #[test]
    fn aer_to_enu_reference() {
        let enu = Aer {
            azimuth_deg: 45.0,
            elevation_deg: 0.0,
            range: Length::from_meters(100.0),
        }
        .to_enu();
        let component = 100.0 * 45.0_f64.to_radians().sin();
        assert_close(enu.east, component, 1e-9);
        assert_close(enu.north, component, 1e-9);
        assert_close(enu.up, 0.0, 1e-9);
    }

    #[test]
    fn aer_enu_round_trip() {
        let aer = Aer {
            azimuth_deg: 120.0,
            elevation_deg: 30.0,
            range: Length::from_meters(5000.0),
        };
        let back = aer.to_enu().to_aer();
        assert_close(back.azimuth_deg, 120.0, 1e-9);
        assert_close(back.elevation_deg, 30.0, 1e-9);
        assert_close(back.range.meters(), 5000.0, 1e-9);
    }

    #[test]
    fn enu_ned_repackaging() {
        let enu = Enu {
            east: 1.0,
            north: 2.0,
            up: 3.0,
        };
        let ned = enu.to_ned();
        assert_close(ned.north, 2.0, 0.0);
        assert_close(ned.east, 1.0, 0.0);
        assert_close(ned.down, -3.0, 0.0);
        // Round-trips back to ENU.
        let enu2 = ned.to_enu();
        assert_close(enu2.up, 3.0, 0.0);
    }

    #[test]
    fn zero_enu_aer_is_well_defined() {
        // A zero offset has zero range and a defined (not NaN) elevation.
        let aer = Enu {
            east: 0.0,
            north: 0.0,
            up: 0.0,
        }
        .to_aer();
        assert_close(aer.range.meters(), 0.0, 0.0);
        assert_close(aer.elevation_deg, 0.0, 0.0);
    }
}
