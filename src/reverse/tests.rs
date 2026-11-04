//! Lookup tests over small datasets built with the `geocoordinates-gazetteer`
//! writer.

use geocoordinates_gazetteer::{Admin, Dataset, FeatureClass as Class, PlaceRow, encode};

use super::*;
use crate::{Approx, Coordinate, Crs, Error, Length, LengthUnit};

/// The dataset behind `data/fixture.gaz`, which the doctests load.
pub(super) fn fixture_dataset() -> Dataset {
    let mut d = Dataset::new("2026-11-03", geocoordinates_gazetteer::GEONAMES_ATTRIBUTION);
    d.languages = vec!["fr".into(), "ja".into()];
    d.admin1 = vec![
        Admin {
            code: "11".into(),
            name: "Île-de-France".into(),
        },
        Admin {
            code: "84".into(),
            name: "Auvergne-Rhône-Alpes".into(),
        },
    ];
    d.admin2 = vec![
        Admin {
            code: "75".into(),
            name: "Paris".into(),
        },
        Admin {
            code: "69".into(),
            name: "Rhône".into(),
        },
    ];
    let mut paris = PlaceRow::new(4_885_341, 234_880, "Paris", *b"FR", Class::Capital);
    paris.population = 2_138_551;
    (paris.admin1, paris.admin2) = (Some(0), Some(0));
    paris.localized = vec![(0, "Paris".into()), (1, "パリ".into())];
    let mut montmartre = PlaceRow::new(4_888_722, 234_083, "Montmartre", *b"FR", Class::Section);
    (montmartre.admin1, montmartre.admin2) = (Some(0), Some(0));
    let mut lyon = PlaceRow::new(4_574_906, 484_789, "Lyon", *b"FR", Class::Admin1Seat);
    lyon.population = 520_774;
    (lyon.admin1, lyon.admin2) = (Some(1), Some(1));
    let mut savusavu = PlaceRow::new(-1_677_942, 17_933_564, "Savusavu", *b"FJ", Class::Locality);
    savusavu.population = 6_833;
    d.places = vec![paris, montmartre, lyon, savusavu];
    d
}

const FIXTURE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/reverse/data/fixture.gaz");

/// The committed doctest fixture is what the writer makes of [`fixture_dataset`].
/// Run with `GEOCOORDINATES_BLESS=1` to rewrite it after changing either.
#[test]
fn the_committed_fixture_is_current() {
    let bytes = encode(&fixture_dataset());
    if std::env::var_os("GEOCOORDINATES_BLESS").is_some() {
        std::fs::write(FIXTURE_PATH, &bytes).unwrap();
    }
    assert_eq!(std::fs::read(FIXTURE_PATH).unwrap(), bytes);
}

fn fixture() -> Gazetteer {
    Gazetteer::from_bytes(&encode(&fixture_dataset())).unwrap()
}

fn wgs(lat: f64, lon: f64) -> Coordinate {
    Coordinate::wgs84(lat, lon).unwrap()
}

fn lookup(g: &Gazetteer, at: Coordinate, options: &ReverseOptions) -> Option<Place> {
    g.reverse(at, options).unwrap()
}

fn name_at(g: &Gazetteer, at: Coordinate, options: &ReverseOptions) -> Option<String> {
    lookup(g, at, options).map(|p| p.name().to_owned())
}

#[test]
fn loads_metadata_and_counts() {
    let g = fixture();
    assert_eq!(g.len(), 4);
    assert!(!g.is_empty());
    assert_eq!(g.source_date(), "2026-11-03");
    assert_eq!(
        g.attribution(),
        geocoordinates_gazetteer::GEONAMES_ATTRIBUTION
    );
    assert_eq!(g.languages(), ["fr", "ja"]);
}

#[test]
fn resolves_every_field_of_the_nearest_place() {
    let g = fixture();
    let at = wgs(48.86, 2.35);
    let place = lookup(&g, at, &ReverseOptions::default()).unwrap();
    assert_eq!(place.name(), "Paris");
    assert_eq!(place.coordinate(), wgs(48.853_41, 2.3488));
    assert_eq!(place.country_code(), "FR");
    assert_eq!(
        (place.admin1_code(), place.admin1_name()),
        (Some("11"), Some("Île-de-France"))
    );
    assert_eq!(
        (place.admin2_code(), place.admin2_name()),
        (Some("75"), Some("Paris"))
    );
    assert_eq!(place.feature_class(), Some(FeatureClass::Capital));
    assert_eq!(place.population(), Some(2_138_551));
    let exact = crate::geodesy::geodesic_distance(at, place.coordinate()).unwrap();
    assert_eq!(place.distance(), exact);
    assert_eq!(*place.query().value(), at);
    assert_eq!(place.query().max_error(), Length::ZERO);
    assert!(place.localized_names().is_empty());
}

#[test]
fn a_place_without_population_or_divisions_reads_as_none() {
    let g = fixture();
    let place = lookup(&g, wgs(48.8872, 2.3408), &ReverseOptions::default()).unwrap();
    assert_eq!(place.name(), "Montmartre");
    assert_eq!(place.population(), None);
    assert_eq!(place.feature_class(), Some(FeatureClass::Section));
    let fiji = ReverseOptions::default().with_max_distance(Length::from_meters(1.0));
    let place = lookup(&g, wgs(-16.779_42, 179.335_64), &fiji).unwrap();
    assert_eq!((place.admin1_code(), place.admin2_name()), (None, None));
    assert_eq!(place.population(), Some(6_833));
}

#[test]
fn the_maximum_distance_is_inclusive_and_never_exceeded() {
    let g = fixture();
    let at = wgs(48.86, 2.35);
    let d = lookup(&g, at, &ReverseOptions::default())
        .unwrap()
        .distance();
    let exactly = ReverseOptions::default().with_max_distance(d);
    assert_eq!(name_at(&g, at, &exactly).as_deref(), Some("Paris"));
    let short =
        ReverseOptions::default().with_max_distance(Length::from_meters(d.meters() * 0.999));
    assert_eq!(name_at(&g, at, &short), None);
    let zero = ReverseOptions::default().with_max_distance(Length::ZERO);
    assert_eq!(
        name_at(&g, wgs(45.749_06, 4.847_89), &zero).as_deref(),
        Some("Lyon")
    );
    // Open ocean: nothing within the default 50 km.
    assert_eq!(
        name_at(&g, wgs(0.0, -30.0), &ReverseOptions::default()),
        None
    );
}

#[test]
fn crosses_the_antimeridian() {
    let g = fixture();
    let wide = ReverseOptions::default()
        .with_max_distance(Length::from_unit(100.0, LengthUnit::Kilometer));
    let place = lookup(&g, wgs(-16.8, -179.9), &wide).unwrap();
    assert_eq!(place.name(), "Savusavu");
    assert!(
        (place.distance().meters() - 81_500.0).abs() < 100.0,
        "{:?}",
        place.distance()
    );
}

#[test]
fn filters_by_feature_class_and_population() {
    let g = fixture();
    let sacre_coeur = wgs(48.8867, 2.3431);
    let any = ReverseOptions::default();
    assert_eq!(
        name_at(&g, sacre_coeur, &any).as_deref(),
        Some("Montmartre")
    );
    let locality = any.clone().with_min_feature_class(FeatureClass::Locality);
    assert_eq!(
        name_at(&g, sacre_coeur, &locality).as_deref(),
        Some("Paris")
    );
    let section = any.clone().with_min_feature_class(FeatureClass::Section);
    assert_eq!(
        name_at(&g, sacre_coeur, &section).as_deref(),
        Some("Montmartre")
    );
    let capital = any.clone().with_min_feature_class(FeatureClass::Capital);
    assert_eq!(name_at(&g, wgs(45.75, 4.85), &capital), None);
    let one = any.clone().with_min_population(1);
    assert_eq!(name_at(&g, sacre_coeur, &one).as_deref(), Some("Paris"));
    let exact = any.clone().with_min_population(2_138_551);
    assert_eq!(name_at(&g, sacre_coeur, &exact).as_deref(), Some("Paris"));
    let more = any.with_min_population(2_138_552);
    assert_eq!(name_at(&g, sacre_coeur, &more), None);
}

#[test]
fn widens_the_search_until_a_place_passes() {
    // 40 sections ring the query, nearer than the one locality; the first k-nearest
    // search sees only sections, so the lookup has to widen it twice.
    let mut d = Dataset::new("d", "a");
    for i in 0..40 {
        let lon = 100 + i;
        d.places.push(PlaceRow::new(
            0,
            lon,
            format!("s{i}"),
            *b"XX",
            Class::Section,
        ));
    }
    d.places
        .push(PlaceRow::new(0, 1_000, "town", *b"XX", Class::Locality));
    let g = Gazetteer::from_bytes(&encode(&d)).unwrap();
    let locality = ReverseOptions::default().with_min_feature_class(FeatureClass::Locality);
    assert_eq!(
        name_at(&g, wgs(0.0, 0.0), &locality).as_deref(),
        Some("town")
    );
    let capital = ReverseOptions::default().with_min_feature_class(FeatureClass::Capital);
    assert_eq!(name_at(&g, wgs(0.0, 0.0), &capital), None);
}

#[test]
fn distance_ties_go_to_the_place_listed_first() {
    let mut d = Dataset::new("d", "a");
    d.places
        .push(PlaceRow::new(0, 100, "first", *b"XX", Class::Locality));
    d.places
        .push(PlaceRow::new(0, -100, "second", *b"XX", Class::Locality));
    let g = Gazetteer::from_bytes(&encode(&d)).unwrap();
    assert_eq!(
        name_at(&g, wgs(0.0, 0.0), &ReverseOptions::default()).as_deref(),
        Some("first")
    );
}

#[test]
fn an_empty_gazetteer_finds_nothing() {
    let g = Gazetteer::from_bytes(&encode(&Dataset::new("d", "a"))).unwrap();
    assert!(g.is_empty());
    assert_eq!(name_at(&g, wgs(0.0, 0.0), &ReverseOptions::default()), None);
}

#[test]
fn localizes_in_the_requested_languages() {
    let g = fixture();
    let at = wgs(48.86, 2.35);
    let both = ReverseOptions::default().with_languages(["JA", "fr-CA", "de"]);
    let place = lookup(&g, at, &both).unwrap();
    let names: Vec<(&str, &str)> = place
        .localized_names()
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(names, [("fr", "Paris"), ("ja", "パリ")]);
    assert_eq!(place.localized_name("ja"), Some("パリ"));
    // A place with no name in a requested language has none.
    let lyon = lookup(&g, wgs(45.75, 4.85), &both).unwrap();
    assert!(lyon.localized_names().is_empty());
}

#[test]
fn rejects_invalid_options() {
    let g = fixture();
    let at = wgs(0.0, 0.0);
    for meters in [-1.0, f64::NAN, f64::INFINITY] {
        let options = ReverseOptions::default().with_max_distance(Length::from_meters(meters));
        assert!(matches!(
            g.reverse(at, &options),
            Err(Error::InvalidValue {
                field: "max_distance",
                ..
            })
        ));
    }
    for tag in ["", "x", "en_US", "english-languages"] {
        let options = ReverseOptions::default().with_languages([tag]);
        assert!(
            matches!(
                g.reverse(at, &options),
                Err(Error::InvalidValue {
                    field: "languages",
                    ..
                })
            ),
            "{tag}"
        );
    }
}

#[test]
fn converts_china_datums_and_carries_the_bound() {
    let g = fixture();
    let paris = wgs(48.853_41, 2.3488);
    // Outside China GCJ-02 is WGS-84, so the conversion is exact.
    let gcj = Coordinate::gcj02(paris.lat(), paris.lon()).unwrap();
    let place = lookup(&g, gcj, &ReverseOptions::default()).unwrap();
    assert_eq!(place.name(), "Paris");
    assert_eq!(
        place.query(),
        crate::convert::convert(gcj, Crs::Wgs84).unwrap()
    );
    assert_eq!(place.query().value().crs(), Crs::Wgs84);
    // BD-09 is offset everywhere; its inverse is approximate.
    let bd = crate::convert::convert(paris, Crs::Bd09)
        .unwrap()
        .into_inner();
    let place = lookup(&g, bd, &ReverseOptions::default()).unwrap();
    assert_eq!(place.name(), "Paris");
    assert!(place.query().max_error().meters() > 0.0);
    assert!(place.distance().meters() < 2.0, "{:?}", place.distance());
    // The BD-09 inverse cannot carry a point at the South Pole.
    let pole = Coordinate::bd09(-90.0, 0.0).unwrap();
    assert!(matches!(
        g.reverse(pole, &ReverseOptions::default()),
        Err(Error::OutsideDomain { .. })
    ));
}

#[test]
fn ignores_height_and_converts_classic_datums() {
    let g = fixture();
    let high = wgs(48.86, 2.35)
        .with_height(crate::Height::Orthometric(300.0))
        .unwrap();
    assert_eq!(
        name_at(&g, high, &ReverseOptions::default()).as_deref(),
        Some("Paris")
    );
    let tokyo = Coordinate::new(48.86, 2.35, Crs::Tokyo).unwrap();
    let place = lookup(&g, tokyo, &ReverseOptions::default()).unwrap();
    assert_eq!(place.name(), "Paris");
    assert_eq!(place.query().value().crs(), Crs::Wgs84);
}

#[test]
fn a_trait_object_dispatches_to_the_gazetteer() {
    let g = fixture();
    let geocoder: &dyn ReverseGeocoder = &g;
    let place = geocoder
        .reverse(wgs(45.75, 4.85), &ReverseOptions::default())
        .unwrap();
    assert_eq!(place.unwrap().name(), "Lyon");
}

#[test]
fn place_new_validates_and_measures() {
    let at = wgs(10.0, 20.0);
    let query = Approx::new(at, Length::from_meters(3.0)).unwrap();
    let place = Place::new("Here", at, "AB", query).unwrap();
    assert_eq!(place.distance(), Length::ZERO);
    assert_eq!(place.query(), query);
    for code in ["", "A", "ab", "ABC", "A1", "ÀB"] {
        assert!(
            matches!(
                Place::new("Here", at, code, query),
                Err(Error::InvalidValue {
                    field: "country_code",
                    ..
                })
            ),
            "{code}"
        );
    }
    let tokyo = Coordinate::new(10.0, 20.0, Crs::Tokyo).unwrap();
    assert!(matches!(
        Place::new("Here", tokyo, "AB", query),
        Err(Error::CrsMismatch { .. })
    ));
}

#[test]
fn localized_names_replace_case_insensitively() {
    let at = wgs(0.0, 0.0);
    let place = Place::new("Here", at, "AB", Approx::new(at, Length::ZERO).unwrap())
        .unwrap()
        .with_localized_name("zh-Hant", "一")
        .unwrap()
        .with_localized_name("ZH-hant", "二")
        .unwrap()
        .with_localized_name("en", "Here")
        .unwrap();
    assert_eq!(place.localized_names().len(), 2);
    assert_eq!(place.localized_name("zh-HANT"), Some("二"));
    assert_eq!(place.localized_name("en"), Some("Here"));
    assert_eq!(place.localized_name("fr"), None);
}

#[test]
fn feature_classes_match_the_writer() {
    let pairs = [
        (Class::Section, FeatureClass::Section),
        (Class::Locality, FeatureClass::Locality),
        (Class::Admin4Seat, FeatureClass::Admin4Seat),
        (Class::Admin3Seat, FeatureClass::Admin3Seat),
        (Class::Admin2Seat, FeatureClass::Admin2Seat),
        (Class::Admin1Seat, FeatureClass::Admin1Seat),
        (Class::Capital, FeatureClass::Capital),
    ];
    for (written, read) in pairs {
        assert_eq!(FeatureClass::from_byte(written as u8), Some(read));
    }
    assert_eq!(FeatureClass::from_byte(7), None);
}

#[test]
fn options_admit_by_both_filters() {
    let options = ReverseOptions::new()
        .with_min_population(10)
        .with_min_feature_class(FeatureClass::Locality);
    assert!(options.admits(FeatureClass::Locality, Some(10)));
    assert!(options.admits(FeatureClass::Capital, Some(11)));
    assert!(!options.admits(FeatureClass::Section, Some(10)));
    assert!(!options.admits(FeatureClass::Locality, Some(9)));
    assert!(!options.admits(FeatureClass::Locality, None));
    assert!(ReverseOptions::new().admits(FeatureClass::Section, None));
}

#[cfg(feature = "serde")]
mod serde_round_trip {
    use super::*;

    fn paris() -> Place {
        fixture()
            .reverse(
                wgs(48.86, 2.35),
                &ReverseOptions::default().with_languages(["ja"]),
            )
            .unwrap()
            .unwrap()
    }

    #[test]
    fn a_place_round_trips_through_json() {
        let place = paris();
        let json = serde_json::to_value(&place).unwrap();
        assert_eq!(json["admin1_name"], "Île-de-France");
        assert_eq!(json["localized_names"]["ja"], "パリ");
        let back: Place = serde_json::from_value(json).unwrap();
        assert_eq!(back, place);
    }

    #[test]
    fn deserializing_a_place_checks_it() {
        let json = serde_json::to_value(paris()).unwrap();
        let reject = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut bad = json.clone();
            edit(&mut bad);
            serde_json::from_value::<Place>(bad)
                .unwrap_err()
                .to_string()
        };
        assert!(reject(&|j| j["country_code"] = "fr".into()).contains("country_code"));
        assert!(reject(&|j| j["admin1_name"] = serde_json::Value::Null).contains("admin1"));
        assert!(reject(&|j| j["admin2_code"] = serde_json::Value::Null).contains("admin2"));
        assert!(reject(&|j| j["distance"]["meters"] = 1.0.into()).contains("distance"));
        assert!(reject(&|j| j["localized_names"]["not a tag"] = "x".into()).contains("language"));
        assert!(reject(&|j| j["name"] = "".into()).contains("name"));
    }

    #[test]
    fn options_round_trip_and_default_missing_fields() {
        let options = ReverseOptions::default()
            .with_languages(["ja"])
            .with_min_feature_class(FeatureClass::Admin1Seat);
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(
            serde_json::from_str::<ReverseOptions>(&json).unwrap(),
            options
        );
        let sparse: ReverseOptions = serde_json::from_str(r#"{"min_population":5}"#).unwrap();
        assert_eq!(sparse, ReverseOptions::default().with_min_population(5));
    }
}
