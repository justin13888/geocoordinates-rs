//! `from_edges`: the per-edge range errors, the extreme edges, and the serde
//! wire shape the edge record shares with `GeoBounds`.

use super::*;

fn bounds(south: f64, west: f64, north: f64, east: f64) -> GeoBounds {
    GeoBounds::from_edges(edges_on(south, west, north, east, Crs::Wgs84)).unwrap()
}

#[test]
fn from_edges_names_the_edge_outside_the_range() {
    for (args, name) in [
        ((-90.5, 0.0, 0.0, 0.0), "south"),
        ((0.0, -180.5, 0.0, 0.0), "west"),
        ((0.0, 0.0, 90.5, 0.0), "north"),
        ((0.0, 0.0, 0.0, 180.5), "east"),
        ((f64::NAN, 0.0, 0.0, 0.0), "south"),
        ((0.0, f64::INFINITY, 0.0, 0.0), "west"),
        ((0.0, 0.0, f64::NEG_INFINITY, 0.0), "north"),
        ((0.0, 0.0, 0.0, f64::NAN), "east"),
    ] {
        let found = GeoBounds::from_edges(edges_on(args.0, args.1, args.2, args.3, Crs::Wgs84));
        assert!(
            matches!(found, Err(Error::InvalidValue { field, .. }) if field == name),
            "{args:?}: {found:?}"
        );
    }
    assert!(matches!(
        GeoBounds::from_edges(edges_on(1.0, 0.0, 0.0, 0.0, Crs::Wgs84)),
        Err(Error::InvalidValue {
            field: "south_west",
            ..
        })
    ));
}

#[test]
fn from_edges_accepts_the_extreme_edges() {
    let b = bounds(-90.0, -180.0, 90.0, 180.0);
    assert_eq!(
        (b.south(), b.west(), b.north(), b.east()),
        (-90.0, -180.0, 90.0, 180.0)
    );
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trips_and_validates() {
    let b = bounds(1.0, 170.0, 2.0, -170.0);
    let json = serde_json::to_string(&b).unwrap();
    let wire = r#"{"south":1.0,"west":170.0,"north":2.0,"east":-170.0,"crs":"Wgs84"}"#;
    assert_eq!(json, wire);
    assert_eq!(serde_json::from_str::<GeoBounds>(&json).unwrap(), b);
    // The edge record is the same wire shape, unchecked.
    let record = edges_on(1.0, 170.0, 2.0, -170.0, Crs::Wgs84);
    assert_eq!(serde_json::to_string(&record).unwrap(), wire);
    assert_eq!(
        serde_json::from_str::<GeoBoundsEdges>(wire).unwrap(),
        record
    );
    let bad = r#"{"south":5.0,"west":0.0,"north":1.0,"east":0.0,"crs":"Wgs84"}"#;
    assert!(serde_json::from_str::<GeoBounds>(bad).is_err());
    let east = r#"{"south":0.0,"west":0.0,"north":1.0,"east":181.0,"crs":"Wgs84"}"#;
    let message = serde_json::from_str::<GeoBounds>(east)
        .unwrap_err()
        .to_string();
    let expected = edges_on(0.0, 0.0, 1.0, 181.0, Crs::Wgs84);
    let expected = GeoBounds::from_edges(expected).unwrap_err().to_string();
    assert!(message.starts_with(&expected), "{message}");
}

/// The edge record of a box, positionally, for the tables in these tests.
pub(super) fn edges_on(south: f64, west: f64, north: f64, east: f64, crs: Crs) -> GeoBoundsEdges {
    GeoBoundsEdges {
        south,
        west,
        north,
        east,
        crs,
    }
}
