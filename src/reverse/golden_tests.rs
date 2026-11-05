//! Golden lookups against the bundled GeoNames `cities1000` gazetteer.
//!
//! Each case pins the place a fixed point resolves to. Regenerating the bundled
//! data may move a case; a change here is then a change in GeoNames, to read
//! before accepting.

use std::sync::OnceLock;

use super::*;
use crate::convert::convert;
use crate::{Coordinate, Crs, Length, LengthUnit};

fn bundled() -> &'static Gazetteer {
    static GAZETTEER: OnceLock<Gazetteer> = OnceLock::new();
    GAZETTEER.get_or_init(Gazetteer::bundled)
}

/// The name, country and admin1 code `at` resolves to with `options`.
fn at_with(at: Coordinate, options: &ReverseOptions) -> Option<(String, String, Option<String>)> {
    bundled().reverse(at, options).unwrap().map(|place| {
        (
            place.name().to_owned(),
            place.country_code().to_owned(),
            place.admin1_code().map(str::to_owned),
        )
    })
}

fn at(lat: f64, lon: f64) -> Option<(String, String, Option<String>)> {
    at_with(
        Coordinate::wgs84(lat, lon).unwrap(),
        &ReverseOptions::default(),
    )
}

fn expect(name: &str, country: &str, admin1: &str) -> Option<(String, String, Option<String>)> {
    Some((name.to_owned(), country.to_owned(), Some(admin1.to_owned())))
}

#[test]
fn the_bundled_data_states_its_source_and_licence() {
    let g = bundled();
    assert_eq!(g.len(), 171_703);
    assert_eq!(g.source_date(), "2026-11-03");
    assert_eq!(
        g.attribution(),
        geocoordinates_gazetteer::GEONAMES_ATTRIBUTION
    );
    assert!(g.languages().is_empty());
}

#[test]
fn cities() {
    assert_eq!(at(48.8534, 2.3488), expect("Paris", "FR", "11"));
    // Paris lists its arrondissements too; the Hôtel de Ville is in the 4th.
    assert_eq!(
        at(48.8566, 2.3522),
        expect("Paris 04 Hôtel-de-Ville", "FR", "11")
    );
    assert_eq!(at(35.6895, 139.6917), expect("Tokyo", "JP", "40"));
    assert_eq!(at(-33.8688, 151.2093), expect("Sydney", "AU", "02"));
    assert_eq!(at(-34.6037, -58.3816), expect("Buenos Aires", "AR", "07"));
}

#[test]
fn border_towns_resolve_to_their_own_side() {
    // Valga, Estonia and Valka, Latvia share one street grid.
    assert_eq!(at(57.778, 26.045), expect("Valga", "EE", "19"));
    assert_eq!(at(57.775, 26.012), expect("Valka", "LV", "30"));
    // Görlitz, Germany and Zgorzelec, Poland face each other across the Neisse.
    assert_eq!(at(51.155, 14.985), expect("Görlitz", "DE", "13"));
    assert_eq!(at(51.149, 15.008), expect("Zgorzelec", "PL", "72"));
}

#[test]
fn islands() {
    assert_eq!(at(32.2949, -64.783), expect("Hamilton", "BM", "03"));
    // GeoNames gives the Falklands no admin1 division.
    let stanley = Some(("Stanley".to_owned(), "FK".to_owned(), None));
    assert_eq!(at(-51.6938, -57.857), stanley);
    assert_eq!(at(78.2232, 15.6267), expect("Longyearbyen", "SJ", "21"));
}

#[test]
fn across_the_antimeridian() {
    // Chukotka, east of 180°: Beringovskiy lies 43 km west, across it.
    let place = bundled()
        .reverse(
            Coordinate::wgs84(63.0, -179.8).unwrap(),
            &ReverseOptions::default(),
        )
        .unwrap()
        .unwrap();
    assert_eq!((place.name(), place.country_code()), ("Beringovskiy", "RU"));
    assert!(
        (place.distance().meters() - 43_400.0).abs() < 1_000.0,
        "{:?}",
        place.distance()
    );
    // Taveuni, Fiji: Savusavu is 82 km west, past the default 50 km.
    let taveuni = Coordinate::wgs84(-16.8, -179.9).unwrap();
    assert_eq!(at_with(taveuni, &ReverseOptions::default()), None);
    let wide = ReverseOptions::default()
        .with_max_distance(Length::from_unit(100.0, LengthUnit::Kilometer));
    assert_eq!(at_with(taveuni, &wide), expect("Savusavu", "FJ", "03"));
}

#[test]
fn open_ocean_and_the_poles_are_nowhere() {
    for (lat, lon) in [
        (0.0, -30.0),
        (30.0, -40.0),
        (-45.0, -120.0),
        (35.0, 160.0),
        (-60.0, 90.0),
    ] {
        assert_eq!(at(lat, lon), None, "{lat}, {lon}");
    }
    assert_eq!(at(90.0, 0.0), None);
    assert_eq!(at(-90.0, 0.0), None);
}

#[test]
fn gcj02_input_resolves_where_it_is_not_where_it_reads() {
    // Each is a GeoNames place center rendered in GCJ-02, as a Chinese map
    // provider reports it, about 600 m from where the same numbers fall in WGS-84.
    for (gcj_lat, gcj_lon, place, misread) in [
        (37.869_944, 112.566_554, "Taiyuan", "Liuxiang"),
        (31.218_156, 121.420_476, "Jing’an", "Changning"),
        (34.256_867, 108.928_824, "Lianhu", "Xi’an"),
        (22.537_618, 114.066_478, "Gangxia", "Gangsha"),
    ] {
        let gcj = Coordinate::gcj02(gcj_lat, gcj_lon).unwrap();
        let found = bundled()
            .reverse(gcj, &ReverseOptions::default())
            .unwrap()
            .unwrap();
        assert_eq!(found.name(), place);
        assert_eq!(found.country_code(), "CN");
        assert!(
            found.distance().meters() < 1.0,
            "{place}: {:?}",
            found.distance()
        );
        assert_eq!(found.query().value().crs(), Crs::Wgs84);
        assert!(found.query().max_error().meters() > 0.0);
        let as_if_wgs = Coordinate::wgs84(gcj_lat, gcj_lon).unwrap();
        assert_eq!(
            at_with(as_if_wgs, &ReverseOptions::default()).unwrap().0,
            misread
        );
    }
}

#[test]
fn bd09_input_resolves_too() {
    let taiyuan = Coordinate::wgs84(37.869_44, 112.560_28).unwrap();
    let bd = convert(taiyuan, Crs::Bd09).unwrap().into_inner();
    let found = bundled()
        .reverse(bd, &ReverseOptions::default())
        .unwrap()
        .unwrap();
    assert_eq!(found.name(), "Taiyuan");
    assert!(found.distance() < found.query().max_error() + Length::from_meters(0.01));
}

#[test]
fn filters_narrow_to_larger_places() {
    // Montmartre, Paris: a neighbourhood first, the capital at the Capital floor.
    let sacre_coeur = Coordinate::wgs84(48.8867, 2.3431).unwrap();
    let capital = ReverseOptions::default().with_min_feature_class(FeatureClass::Capital);
    assert_eq!(at_with(sacre_coeur, &capital), expect("Paris", "FR", "11"));
    let big = ReverseOptions::default().with_min_population(1_000_000);
    assert_eq!(at_with(sacre_coeur, &big), expect("Paris", "FR", "11"));
}
