//! The photo-library seam, end to end: GPS tag values a reader extracted become
//! fixes, the fixes become a track that places an untagged photo, the placed fix
//! is labelled by reverse geocoding, and the fix shared from it is coarsened
//! inside an exclusion zone. Each step's own rules are unit-tested in its module;
//! this pins only that their types and promises compose.
#![cfg(all(
    feature = "exif",
    feature = "geotag",
    feature = "privacy",
    feature = "reverse"
))]

use std::time::Duration;

use geocoordinates::exif::{CaptureGeo, ExifGps, ExifRational};
use geocoordinates::fix::{AccuracyKind, Fix};
use geocoordinates::geodesy::geodesic_distance;
use geocoordinates::geotag::{GeotagOptions, Track};
use geocoordinates::privacy::{ExclusionZone, Precision, ZoneAction, apply};
use geocoordinates::reverse::{Gazetteer, ReverseGeocoder, ReverseOptions};
use geocoordinates::{Coordinate, Crs, Length};
use geocoordinates_gazetteer::{Dataset, FeatureClass, PlaceRow, encode};

/// An EXIF rational with the given numerator over 100.
fn hundredths(value: u32) -> ExifRational {
    ExifRational::Unsigned(value, 100)
}

/// The GPS block of a phone photo in Paris (north and east), taken at
/// 12:`minute`:00 UTC on 2026-06-01 with a 5 m horizontal positioning error.
fn tagged(lat_dms: [u32; 3], lon_dms: [u32; 3], minute: u32) -> ExifGps {
    let dms = |[d, m, s]: [u32; 3]| [hundredths(d * 100), hundredths(m * 100), hundredths(s)];
    let mut gps = ExifGps::default();
    gps.latitude = Some(dms(lat_dms));
    gps.latitude_ref = Some("N".into());
    gps.longitude = Some(dms(lon_dms));
    gps.longitude_ref = Some("E\0".into());
    gps.map_datum = Some("WGS-84".into());
    gps.h_positioning_error = Some(hundredths(500));
    gps.date_stamp = Some("2026:06:01".into());
    gps.time_stamp = Some([hundredths(1_200), hundredths(minute * 100), hundredths(0)]);
    gps
}

/// A gazetteer holding Paris and Lyon, written by the format's own writer.
fn gazetteer() -> Gazetteer {
    let mut dataset = Dataset::new("2026-06-01", "test");
    dataset.places = vec![
        PlaceRow::new(4_885_341, 234_880, "Paris", *b"FR", FeatureClass::Capital),
        PlaceRow::new(4_574_906, 484_789, "Lyon", *b"FR", FeatureClass::Admin1Seat),
    ];
    Gazetteer::from_bytes(&encode(&dataset)).unwrap()
}

#[test]
fn tagged_photos_place_label_and_coarsen_an_untagged_one() {
    // Three tagged photos on a walk: 48.8566, 2.3522 / 48.86, 2.36 / 48.864, 2.37.
    let fixes: Vec<Fix> = [
        tagged([48, 51, 2_376], [2, 21, 792], 0),
        tagged([48, 51, 3_600], [2, 21, 3_600], 2),
        tagged([48, 51, 5_040], [2, 22, 1_200], 4),
    ]
    .iter()
    .map(|gps| CaptureGeo::from_exif(gps).unwrap().fix)
    .collect();
    let first = fixes[0].clone();
    assert!((first.coord.lat() - 48.8566).abs() < 1e-9);
    assert!((first.coord.lon() - 2.3522).abs() < 1e-9);
    assert_eq!(first.coord.crs(), Crs::Wgs84);
    assert_eq!(first.accuracy.unwrap().kind(), AccuracyKind::Bound);

    // A camera photo with no GPS, taken a minute after the first.
    let track = Track::new(fixes.clone()).unwrap();
    let options = GeotagOptions::default().with_max_gap(Duration::from_secs(300));
    let shot_at = first.timestamp.unwrap() + Duration::from_secs(60);
    let placed = track.locate(shot_at, options).unwrap().unwrap();
    assert_eq!(placed.timestamp, Some(shot_at));
    assert!(
        placed
            .source
            .as_ref()
            .unwrap()
            .raw
            .starts_with("geotag:interpolated")
    );
    let along = geodesic_distance(first.coord, placed.coord).unwrap();
    let whole = geodesic_distance(first.coord, fixes[1].coord).unwrap();
    assert!((along.meters() - whole.meters() / 2.0).abs() < 1e-6);
    let bound = placed.accuracy.unwrap();
    assert_eq!(bound.kind(), AccuracyKind::Bound);

    // It is labelled with the nearest place.
    let places = gazetteer();
    let label = places
        .reverse(placed.coord, &ReverseOptions::default())
        .unwrap()
        .unwrap();
    assert_eq!((label.name(), label.country_code()), ("Paris", "FR"));

    // Home is a zone around where it was taken: the shared fix is coarsened.
    let home = Coordinate::wgs84(48.8583, 2.3561).unwrap();
    let zones = [ExclusionZone::from_circle(home, Length::from_meters(300.0)).unwrap()];
    let action = ZoneAction::Coarsen(Precision::geohash(5).unwrap());
    let shared = apply(placed.clone(), &zones, action).unwrap().unwrap();
    assert_ne!(shared.coord, placed.coord);
    assert_eq!(shared.timestamp, placed.timestamp);
    assert!(
        shared
            .source
            .as_ref()
            .unwrap()
            .raw
            .starts_with("privacy:geohash:")
    );
    let shared_bound = shared.accuracy.unwrap().horizontal().unwrap();
    let moved = geodesic_distance(placed.coord, shared.coord).unwrap();
    assert!(moved <= shared_bound, "{moved:?} > {shared_bound:?}");
    assert!(shared_bound > bound.horizontal().unwrap());
    // The coarse fix still labels the city; a photo outside the zone is shared as is.
    let coarse_label = places.reverse(shared.coord, &ReverseOptions::default());
    assert_eq!(coarse_label.unwrap().unwrap().name(), "Paris");
    let last = fixes[2].clone();
    assert_eq!(apply(last.clone(), &zones, action).unwrap(), Some(last));
}
