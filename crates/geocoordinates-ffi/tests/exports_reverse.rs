//! Calls every reverse-geocoding FFI export at least once with an asserted value,
//! including the failure each one documents.

use std::sync::Arc;

use geocoordinates_ffi::*;

/// The core crate's doctest gazetteer: Paris, Montmartre, Lyon and Savusavu, with
/// French and Japanese names for Paris.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/reverse/data/fixture.gaz"
);

fn wgs84(lat: f64, lon: f64) -> Coordinate {
    coordinate_new(lat, lon, Crs::Wgs84).unwrap()
}

fn fixture() -> Arc<Gazetteer> {
    Gazetteer::from_bytes(std::fs::read(FIXTURE).unwrap()).unwrap()
}

fn paris(options: ReverseOptions) -> Place {
    fixture()
        .reverse(wgs84(48.86, 2.35), options)
        .unwrap()
        .unwrap()
}

#[test]
fn loads_from_bytes_or_a_path() {
    let g = fixture();
    assert_eq!(g.len(), 4);
    assert!(!g.is_empty());
    assert_eq!(g.source_date(), "2026-11-03");
    assert!(g.attribution().contains("GeoNames"));
    assert_eq!(g.languages(), ["fr", "ja"]);
    assert_eq!(Gazetteer::open(FIXTURE.to_owned()).unwrap().len(), 4);
    assert!(matches!(
        Gazetteer::open("/no/such/file.gaz".to_owned()),
        Err(GeoError::InvalidValue { field, .. }) if field == "path"
    ));
    let mut corrupt = std::fs::read(FIXTURE).unwrap();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(matches!(
        Gazetteer::from_bytes(corrupt),
        Err(GeoError::Parse {
            kind: ParseErrorKind::Checksum,
            ..
        })
    ));
}

#[test]
fn resolves_a_place_with_every_field() {
    let options = ReverseOptions {
        languages: vec!["ja".into()],
        ..reverse_options_default()
    };
    let place = paris(options);
    assert_eq!(place.name, "Paris");
    assert_eq!(place.country_code, "FR");
    assert_eq!(place.admin1_name.as_deref(), Some("Île-de-France"));
    assert_eq!(place.admin2_code.as_deref(), Some("75"));
    assert_eq!(place.feature_class, Some(FeatureClass::Capital));
    assert_eq!(place.population, Some(2_138_551));
    assert!((place.distance_m - 738.1).abs() < 0.1);
    assert_eq!(place.query, wgs84(48.86, 2.35));
    assert_eq!(place.query_max_error_m, 0.0);
    assert_eq!(
        place.localized_names.get("ja").map(String::as_str),
        Some("パリ")
    );
    assert!(place_validate(place).is_ok());
}

#[test]
fn options_filter_and_bound_the_search() {
    let defaults = reverse_options_default();
    assert_eq!(defaults.max_distance_m, 50_000.0);
    assert_eq!(
        (defaults.min_population, defaults.min_feature_class),
        (0, None)
    );
    assert!(defaults.languages.is_empty());
    let g = fixture();
    let sacre_coeur = wgs84(48.8867, 2.3431);
    let near = g.reverse(sacre_coeur, defaults.clone()).unwrap().unwrap();
    assert_eq!(near.name, "Montmartre");
    let locality = ReverseOptions {
        min_feature_class: Some(FeatureClass::Locality),
        ..defaults.clone()
    };
    assert_eq!(
        g.reverse(sacre_coeur, locality).unwrap().unwrap().name,
        "Paris"
    );
    let ocean = g.reverse(wgs84(0.0, -30.0), defaults.clone()).unwrap();
    assert_eq!(ocean, None);
    let negative = ReverseOptions {
        max_distance_m: -1.0,
        ..defaults.clone()
    };
    assert!(matches!(
        g.reverse(sacre_coeur, negative),
        Err(GeoError::InvalidValue { field, .. }) if field == "max_distance"
    ));
    let bad_record = Coordinate {
        lat: 91.0,
        ..sacre_coeur
    };
    assert!(g.reverse(bad_record, defaults).is_err());
}

#[test]
fn gcj02_input_carries_its_bound() {
    let bd = coordinate_new(48.86, 2.35, Crs::Bd09).unwrap();
    let place = fixture()
        .reverse(bd, reverse_options_default())
        .unwrap()
        .unwrap();
    assert_eq!(place.query.crs, Crs::Wgs84);
    assert!(place.query_max_error_m > 0.0);
}

#[test]
fn place_new_builds_and_validate_checks() {
    let at = wgs84(10.0, 20.0);
    let place = place_new("Here".into(), at, "AB".into(), at, 0.5).unwrap();
    assert_eq!((place.distance_m, place.query_max_error_m), (0.0, 0.5));
    assert!(place_validate(place.clone()).is_ok());
    assert!(matches!(
        place_new("Here".into(), at, "abc".into(), at, 0.0),
        Err(GeoError::InvalidValue { field, .. }) if field == "country_code"
    ));
    assert!(matches!(
        place_new("Here".into(), at, "AB".into(), at, -1.0),
        Err(GeoError::InvalidValue { field, .. }) if field == "max_error"
    ));
    let gcj = coordinate_new(10.0, 20.0, Crs::Gcj02).unwrap();
    assert!(matches!(
        place_new("Here".into(), gcj, "AB".into(), gcj, 0.0),
        Err(GeoError::CrsMismatch { .. })
    ));

    let field = |bad: Place| match place_validate(bad) {
        Err(GeoError::InvalidValue { field, .. }) => field,
        other => panic!("expected InvalidValue, got {other:?}"),
    };
    let half = Place {
        admin1_code: Some("1".into()),
        ..place.clone()
    };
    assert_eq!(field(half), "admin1");
    let half = Place {
        admin2_name: Some("Two".into()),
        ..place.clone()
    };
    assert_eq!(field(half), "admin2");
    let far = Place {
        distance_m: 5.0,
        ..place.clone()
    };
    assert_eq!(field(far), "distance");
    let nan = Place {
        distance_m: f64::NAN,
        ..place.clone()
    };
    assert_eq!(field(nan), "distance");
    let mut tagged = place.clone();
    tagged
        .localized_names
        .insert("not a tag".into(), "x".into());
    assert_eq!(field(tagged), "language");
    let full = Place {
        admin1_code: Some("1".into()),
        admin1_name: Some("One".into()),
        admin2_code: Some("2".into()),
        admin2_name: Some("Two".into()),
        feature_class: Some(FeatureClass::Admin4Seat),
        population: Some(7),
        ..place
    };
    assert!(place_validate(full).is_ok());
}

/// A geocoder the way a foreign language would write one: here, one that always
/// answers with the same place, valid or not.
struct Fixed(Option<Place>);

impl ReverseGeocoder for Fixed {
    fn reverse(
        &self,
        _coordinate: Coordinate,
        _options: ReverseOptions,
    ) -> Result<Option<Place>, GeoError> {
        Ok(self.0.clone())
    }
}

struct Failing;

impl ReverseGeocoder for Failing {
    fn reverse(
        &self,
        _coordinate: Coordinate,
        _options: ReverseOptions,
    ) -> Result<Option<Place>, GeoError> {
        Err(GeoError::Other {
            detail: "offline".into(),
        })
    }
}

#[test]
fn reverse_geocode_calls_any_implementation_and_checks_it() {
    let at = wgs84(1.0, 2.0);
    let options = reverse_options_default();
    let gazetteer = gazetteer_as_reverse_geocoder(fixture());
    let lyon = reverse_geocode(gazetteer, wgs84(45.75, 4.85), options.clone()).unwrap();
    assert_eq!(lyon.unwrap().name, "Lyon");

    let good = place_new("Here".into(), at, "AB".into(), at, 0.0).unwrap();
    let found = reverse_geocode(Arc::new(Fixed(Some(good.clone()))), at, options.clone());
    assert_eq!(found.unwrap(), Some(good.clone()));
    assert_eq!(
        reverse_geocode(Arc::new(Fixed(None)), at, options.clone()).unwrap(),
        None
    );

    let lying = Place {
        country_code: "nowhere".into(),
        ..good
    };
    assert!(matches!(
        reverse_geocode(Arc::new(Fixed(Some(lying))), at, options.clone()),
        Err(GeoError::InvalidValue { field, .. }) if field == "country_code"
    ));
    assert!(matches!(
        reverse_geocode(Arc::new(Failing), at, options.clone()),
        Err(GeoError::Other { detail }) if detail == "offline"
    ));
    let bad_record = Coordinate { lat: 91.0, ..at };
    assert!(reverse_geocode(Arc::new(Fixed(None)), bad_record, options).is_err());
}

#[test]
fn feature_classes_cross_both_ways() {
    for class in [
        FeatureClass::Section,
        FeatureClass::Locality,
        FeatureClass::Admin4Seat,
        FeatureClass::Admin3Seat,
        FeatureClass::Admin2Seat,
        FeatureClass::Admin1Seat,
        FeatureClass::Capital,
    ] {
        let options = ReverseOptions {
            min_feature_class: Some(class),
            max_distance_m: 1e7,
            ..reverse_options_default()
        };
        let found = fixture().reverse(wgs84(48.8867, 2.3431), options).unwrap();
        let found_class = found.and_then(|p| p.feature_class);
        assert!(
            found_class.is_none_or(|c| c as u8 >= class as u8),
            "{class:?}"
        );
    }
    let at = wgs84(0.0, 0.0);
    for class in [
        FeatureClass::Admin3Seat,
        FeatureClass::Admin2Seat,
        FeatureClass::Locality,
    ] {
        let place = Place {
            feature_class: Some(class),
            ..place_new("Here".into(), at, "AB".into(), at, 0.0).unwrap()
        };
        let back = reverse_geocode(Arc::new(Fixed(Some(place))), at, reverse_options_default());
        assert_eq!(back.unwrap().unwrap().feature_class, Some(class));
    }
}

#[test]
fn unexpected_callback_errors_become_other() {
    let e = GeoError::from(uniffi::UnexpectedUniFFICallbackError::new("boom"));
    assert_eq!(
        e,
        GeoError::Other {
            detail: "boom".into()
        }
    );
}
