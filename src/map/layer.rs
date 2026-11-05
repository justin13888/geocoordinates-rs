//! [`points_to_geojson`] and [`clusters_to_geojson`], re-exported flat from
//! [`map`](super) under the `geojson` feature.

use geojson::feature::Id;
use geojson::{Feature, FeatureCollection, Geometry, JsonObject, JsonValue};

use super::display::{DisplayDatum, display_coordinate};
use crate::cluster::{ClusterIndex, MapCluster};
use crate::coord::Coordinate;
use crate::error::Result;

/// A JSON number from an id. Every `u64` is one.
fn number(id: u64) -> JsonValue {
    JsonValue::from(id)
}

/// A point feature at `point`, drawn in `display`, as `[lon, lat]`.
fn feature(
    point: Coordinate,
    display: DisplayDatum,
    id: Option<u64>,
    properties: JsonObject,
) -> Result<Feature> {
    let shown = display_coordinate(point, display)?;
    Ok(Feature {
        bbox: None,
        geometry: Some(Geometry::new_point([shown.lon(), shown.lat()])),
        id: id.map(|id| match number(id) {
            JsonValue::Number(n) => Id::Number(n),
            _ => unreachable!("a u64 is a JSON number"),
        }),
        properties: Some(properties),
        foreign_members: None,
    })
}

/// supercluster's `point_count_abbreviated`: the count below 1,000, then
/// thousands to one decimal (`"1.2k"`), then whole thousands (`"12k"`).
fn abbreviated(count: u64) -> JsonValue {
    if count >= 10_000 {
        JsonValue::from(format!("{}k", (count as f64 / 1000.0).round()))
    } else if count >= 1_000 {
        JsonValue::from(format!("{}k", (count as f64 / 100.0).round() / 10.0))
    } else {
        number(count)
    }
}

/// A GeoJSON `FeatureCollection` with one `Point` feature per point, in input order,
/// each with a top-level `id` and a `point_id` property holding its position in
/// `points`. Coordinates are `[lon, lat]`, as GeoJSON orders them, drawn in
/// `display`; heights are left out.
///
/// GeoJSON (RFC 7946) positions are WGS-84; a GCJ-02 layer breaks that on purpose,
/// for the China basemaps that expect it, and must be fed to nothing else.
///
/// # Errors
///
/// [`Error::CrsMismatch`](crate::Error::CrsMismatch) unless every point is on
/// WGS-84.
///
/// ```
/// use geocoordinates::{Coordinate, Error, map::{DisplayDatum, points_to_geojson}};
///
/// let layer = points_to_geojson(&[Coordinate::wgs84(48.5, 2.25)?], DisplayDatum::Wgs84)?;
/// assert_eq!(
///     layer,
///     r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[2.25,48.5]},"id":0,"properties":{"point_id":0}}]}"#,
/// );
/// assert!(matches!(
///     points_to_geojson(&[Coordinate::gcj02(48.5, 2.25)?], DisplayDatum::Wgs84),
///     Err(Error::CrsMismatch { .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn points_to_geojson(points: &[Coordinate], display: DisplayDatum) -> Result<String> {
    let features = points
        .iter()
        .enumerate()
        .map(|(i, point)| {
            let id = i as u64;
            let mut properties = JsonObject::new();
            properties.insert("point_id".into(), number(id));
            feature(*point, display, Some(id), properties)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(collection(features))
}

/// The collection's JSON text. Built field by field: the crate's collecting
/// constructor writes an empty `bbox` for no features, which RFC 7946 does not allow.
fn collection(features: Vec<Feature>) -> String {
    FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    }
    .to_string()
}

/// A GeoJSON `FeatureCollection` of map pins in the shape supercluster and
/// MapLibre's clustered sources emit, so a web client can feed it straight to a
/// source, in the order of `clusters`. Coordinates are `[lon, lat]`, at each
/// cluster's centroid, drawn in `display`.
///
/// - A cluster of two or more points has the top-level `id` and the property
///   `cluster_id` equal to [`MapCluster::id`], and the properties `cluster: true`,
///   `point_count` and `point_count_abbreviated` (`"1.2k"` from 1,000 up).
/// - A single point has no `point_count`, which is how a style tells it apart
///   (`["has", "point_count"]`), and no top-level `id`, which could repeat a
///   cluster's; its property `point_id` is its id in `index`.
///
/// # Errors
///
/// [`Error::InvalidValue`](crate::Error::InvalidValue) (`field: "cluster_id"`) for
/// a cluster that `index` does not hold: every pin, of one point or many, must be
/// the cluster `index` reports under its id, so a pin from another index or from
/// before an update is refused even where its id names a cluster here, and
/// [`Error::CrsMismatch`](crate::Error::CrsMismatch) unless the index is on WGS-84.
///
/// ```
/// use geocoordinates::{Coordinate, Crs, Error};
/// use geocoordinates::cluster::ClusterIndex;
/// use geocoordinates::index::GeoBounds;
/// use geocoordinates::map::{DisplayDatum, clusters_to_geojson};
///
/// let index = ClusterIndex::new(
///     [Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(0.0, 0.0)?, Coordinate::wgs84(45.0, 90.0)?],
///     Default::default(),
/// )?;
/// let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84)?;
/// let pins = index.clusters(world, 3)?;
/// let layer = clusters_to_geojson(&index, &pins, DisplayDatum::Wgs84)?;
/// assert!(layer.contains(r#""cluster":true"#) && layer.contains(r#""point_count":2"#));
/// assert!(layer.contains(r#""point_id":2"#));
///
/// let other = ClusterIndex::new([Coordinate::wgs84(1.0, 1.0)?], Default::default())?;
/// assert!(matches!(
///     clusters_to_geojson(&other, &pins, DisplayDatum::Wgs84),
///     Err(Error::InvalidValue { field: "cluster_id", .. })
/// ));
/// # Ok::<(), geocoordinates::Error>(())
/// ```
pub fn clusters_to_geojson(
    index: &ClusterIndex,
    clusters: &[MapCluster],
    display: DisplayDatum,
) -> Result<String> {
    let features = clusters
        .iter()
        .map(|pin| {
            index.check_pin(pin)?;
            let mut properties = JsonObject::new();
            let id = if pin.count() > 1 {
                properties.insert("cluster".into(), JsonValue::Bool(true));
                properties.insert("cluster_id".into(), number(pin.id()));
                properties.insert("point_count".into(), number(pin.count()));
                properties.insert("point_count_abbreviated".into(), abbreviated(pin.count()));
                Some(pin.id())
            } else {
                let leaf = index.leaves(pin.id(), 1, 0)?;
                properties.insert("point_id".into(), number(leaf[0]));
                None
            };
            feature(*pin.centroid().value(), display, id, properties)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(collection(features))
}

#[cfg(test)]
mod tests {
    use super::{abbreviated, clusters_to_geojson, points_to_geojson};
    use crate::cluster::ClusterIndex;
    use crate::index::GeoBounds;
    use crate::map::DisplayDatum;
    use crate::{Coordinate, Crs};

    fn china() -> Vec<Coordinate> {
        [
            (39.9087, 116.3975),
            (39.9087, 116.3975),
            (31.2304, 121.4737),
        ]
        .into_iter()
        .map(|(lat, lon)| Coordinate::wgs84(lat, lon).unwrap())
        .collect()
    }

    #[test]
    fn golden_cluster_layers() {
        let index = ClusterIndex::new(china(), Default::default()).unwrap();
        let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84).unwrap();
        let pins = index.clusters(world, 4).unwrap();
        assert_eq!(
            clusters_to_geojson(&index, &pins, DisplayDatum::Wgs84).unwrap(),
            GOLDEN_WGS84
        );
        assert_eq!(
            clusters_to_geojson(&index, &pins, DisplayDatum::Gcj02).unwrap(),
            GOLDEN_GCJ02
        );
        assert_eq!(
            clusters_to_geojson(&index, &[], DisplayDatum::Wgs84).unwrap(),
            r#"{"type":"FeatureCollection","features":[]}"#
        );
    }

    #[test]
    fn refuses_every_foreign_pin_whose_id_resolves_here() {
        let index = ClusterIndex::new(china(), Default::default()).unwrap();
        let elsewhere: Vec<_> = [(48.85, 2.35), (48.85, 2.35), (40.71, -74.0)]
            .into_iter()
            .map(|(lat, lon)| Coordinate::wgs84(lat, lon).unwrap())
            .collect();
        let other = ClusterIndex::new(elsewhere, Default::default()).unwrap();
        let world = GeoBounds::new(-90.0, -180.0, 90.0, 180.0, Crs::Wgs84).unwrap();
        let (ours, theirs) = (
            index.clusters(world, 4).unwrap(),
            other.clusters(world, 4).unwrap(),
        );
        // The same seeds give the same ids, so each foreign id names a cluster here.
        let ids = |pins: &[crate::cluster::MapCluster]| {
            let mut ids: Vec<_> = pins.iter().map(|pin| (pin.id(), pin.count())).collect();
            ids.sort_unstable();
            ids
        };
        assert_eq!(ids(&ours), ids(&theirs));
        for pin in &theirs {
            assert!(
                matches!(
                    clusters_to_geojson(&index, &[*pin], DisplayDatum::Wgs84),
                    Err(crate::Error::InvalidValue {
                        field: "cluster_id",
                        ..
                    })
                ),
                "{} point(s)",
                pin.count()
            );
        }
    }

    #[test]
    fn golden_point_layer() {
        assert_eq!(
            points_to_geojson(&china()[1..], DisplayDatum::Gcj02).unwrap(),
            GOLDEN_POINTS_GCJ02
        );
    }

    // The two Beijing points are one cluster at zoom 4, with Shanghai alone: the
    // cluster carries supercluster's properties and an `id` (zoom 4 << 32 | seed
    // 0), the lone point only its `point_id`. Positions are `[lon, lat]`.
    const GOLDEN_WGS84: &str = concat!(
        r#"{"type":"FeatureCollection","features":["#,
        r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[116.3975,39.9087]},"#,
        r#""id":17179869184,"properties":{"cluster":true,"cluster_id":17179869184,"#,
        r#""point_count":2,"point_count_abbreviated":2}},"#,
        r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[121.4737,31.2304]},"#,
        r#""properties":{"point_id":2}}]}"#,
    );
    // The same layer for a GCJ-02 basemap: only the positions move.
    const GOLDEN_GCJ02: &str = concat!(
        r#"{"type":"FeatureCollection","features":["#,
        r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[116.40374357265176,39.91010349934476]},"#,
        r#""id":17179869184,"properties":{"cluster":true,"cluster_id":17179869184,"#,
        r#""point_count":2,"point_count_abbreviated":2}},"#,
        r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[121.47822305927693,31.22845773757727]},"#,
        r#""properties":{"point_id":2}}]}"#,
    );
    // GCJ-02 positions from the crate's exact forward offset (`Wgs84::to_gcj02`).
    const GOLDEN_POINTS_GCJ02: &str = concat!(
        r#"{"type":"FeatureCollection","features":["#,
        r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[116.40374357265176,39.91010349934476]},"#,
        r#""id":0,"properties":{"point_id":0}},"#,
        r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[121.47822305927693,31.22845773757727]},"#,
        r#""id":1,"properties":{"point_id":1}}]}"#,
    );

    #[test]
    fn abbreviates_as_supercluster_does() {
        // supercluster 8.0.1, `getClusterProperties`: 999, 1.0k as 1k, 1.25k rounds
        // up, 9,950 to 10k, then whole thousands.
        for (count, expected) in [
            (999, "999"),
            (1_000, "\"1k\""),
            (1_250, "\"1.3k\""),
            (1_249, "\"1.2k\""),
            (9_950, "\"10k\""),
            (10_000, "\"10k\""),
            (12_500, "\"13k\""),
        ] {
            assert_eq!(abbreviated(count).to_string(), expected, "{count}");
        }
    }
}
