//! Pins of the type-level promises `AGENTS.md` makes across the whole public
//! surface: every public type deriving `Hash` hashes consistently with `Eq`,
//! the core and option types are `Send + Sync`, and (under `serde`) every
//! valid-by-construction type deserializes exactly what its constructor
//! accepts. Each test names the promise it holds, so a rename or move that
//! breaks one fails here rather than only in prose.

use crate::angle::{Axis, Hemisphere};
use crate::fix::{AccuracyKind, AxisOrder, DatumAmbiguity, Fix};
use crate::format::{
    DecimalSeparator, FormatOptions, HemisphereStyle, Representation, SymbolStyle,
};
use crate::grids::{Geohash, Maidenhead, Mgrs, MgrsPrecision, PlusCode, UtmHemisphere};
use crate::parse::{GridKind, ParseOptions};
use crate::test_support::assert_hash_consistent;
use crate::{Approx, Coordinate, Crs, Error, LengthUnit, ParseErrorKind};

#[cfg(feature = "serde")]
mod serde_agrees;

/// Compiles only when `T` is `Send + Sync`.
fn assert_send_sync<T: Send + Sync>() {}

/// The error, the core values, `Fix`, and every option struct cross threads.
#[test]
fn public_types_are_send_and_sync() {
    assert_send_sync::<Error>();
    assert_send_sync::<Coordinate>();
    assert_send_sync::<Approx<Coordinate>>();
    assert_send_sync::<Fix>();
    assert_send_sync::<FormatOptions>();
    assert_send_sync::<ParseOptions>();
    #[cfg(feature = "index")]
    {
        assert_send_sync::<crate::index::GeoBounds>();
        assert_send_sync::<crate::index::SpatialIndex<u32>>();
    }
    #[cfg(feature = "exif")]
    assert_send_sync::<crate::exif::ExifOptions>();
    #[cfg(feature = "cluster")]
    assert_send_sync::<crate::cluster::ClusterIndexOptions>();
    #[cfg(feature = "reverse")]
    assert_send_sync::<crate::reverse::ReverseOptions>();
    #[cfg(feature = "geotag")]
    assert_send_sync::<crate::geotag::GeotagOptions>();
}

/// Every unit-variant enum of the core surface: each variant, twice.
#[test]
fn core_enums_hash_consistently() {
    assert_hash_consistent(&[Axis::Latitude, Axis::Longitude, Axis::Latitude]);
    let hemispheres = [
        Hemisphere::North,
        Hemisphere::South,
        Hemisphere::East,
        Hemisphere::West,
    ];
    assert_hash_consistent(&[hemispheres, hemispheres].concat());
    let crs = [
        Crs::Wgs84,
        Crs::Gcj02,
        Crs::Bd09,
        Crs::Nad27,
        Crs::Tokyo,
        Crs::Pulkovo42,
    ];
    assert_hash_consistent(&[crs, crs].concat());
    let units = [
        LengthUnit::Meter,
        LengthUnit::Kilometer,
        LengthUnit::Foot,
        LengthUnit::UsSurveyFoot,
        LengthUnit::NauticalMile,
        LengthUnit::Mile,
        LengthUnit::Yard,
    ];
    assert_hash_consistent(&[units, units].concat());
    let kinds = [
        ParseErrorKind::Syntax,
        ParseErrorKind::Checksum,
        ParseErrorKind::NoFix,
        ParseErrorKind::Ambiguous,
        ParseErrorKind::Unsupported,
        ParseErrorKind::TooDeep,
    ];
    assert_hash_consistent(&[kinds, kinds].concat());
    let accuracy = [
        AccuracyKind::Bound,
        AccuracyKind::OneSigma,
        AccuracyKind::Cep50,
        AccuracyKind::Unspecified,
    ];
    assert_hash_consistent(&[accuracy, accuracy].concat());
    assert_hash_consistent(&[AxisOrder::LatLon, AxisOrder::LonLat, AxisOrder::LatLon]);
    assert_hash_consistent(&[DatumAmbiguity::PossiblyGcj02, DatumAmbiguity::PossiblyGcj02]);
    let grids = [
        GridKind::PlusCode,
        GridKind::Geohash,
        GridKind::Maidenhead,
        GridKind::Mgrs,
    ];
    assert_hash_consistent(&[grids, grids].concat());
    let precisions = [
        MgrsPrecision::Km100,
        MgrsPrecision::Km10,
        MgrsPrecision::Km1,
        MgrsPrecision::M100,
        MgrsPrecision::M10,
        MgrsPrecision::M1,
    ];
    assert_hash_consistent(&[precisions, precisions].concat());
    assert_hash_consistent(&[
        UtmHemisphere::North,
        UtmHemisphere::South,
        UtmHemisphere::North,
    ]);
}

/// The formatting and parsing options, and the enums they are built from.
#[test]
fn format_and_parse_options_hash_consistently() {
    let separators = [DecimalSeparator::Point, DecimalSeparator::Comma];
    assert_hash_consistent(&[separators, separators].concat());
    let symbols = [
        SymbolStyle::Unicode,
        SymbolStyle::Ascii,
        SymbolStyle::Letters,
    ];
    assert_hash_consistent(&[symbols, symbols].concat());
    let styles = [HemisphereStyle::Signed, HemisphereStyle::Cardinal];
    assert_hash_consistent(&[styles, styles].concat());
    let (hemisphere, symbols) = (HemisphereStyle::Cardinal, SymbolStyle::Ascii);
    let representations = [
        Representation::DecimalDegrees {
            decimals: Some(4),
            hemisphere,
        },
        Representation::DecimalDegrees {
            decimals: None,
            hemisphere,
        },
        Representation::Dms {
            second_decimals: Some(1),
            symbols,
            hemisphere,
        },
        Representation::Ddm {
            minute_decimals: None,
            symbols,
            hemisphere,
        },
        Representation::PlusCode { length: 10 },
        Representation::Geohash { length: 7 },
        Representation::Maidenhead { pairs: 3 },
        Representation::Mgrs {
            precision: MgrsPrecision::M10,
        },
    ];
    assert_hash_consistent(&[representations, representations].concat());
    let format_options = |separator| {
        FormatOptions::default()
            .with_representation(Representation::Geohash { length: 7 })
            .with_decimal_separator(separator)
    };
    assert_hash_consistent(&[
        FormatOptions::default(),
        format_options(DecimalSeparator::Comma),
        format_options(DecimalSeparator::Point),
        FormatOptions::default(),
        format_options(DecimalSeparator::Comma),
    ]);
    let parse_options = |grid| {
        ParseOptions::default()
            .with_default_axis_order(AxisOrder::LonLat)
            .with_decimal_separator(DecimalSeparator::Comma)
            .with_expect_grid(grid)
    };
    assert_hash_consistent(&[
        ParseOptions::default(),
        parse_options(Some(GridKind::Mgrs)),
        parse_options(None),
        ParseOptions::default(),
        parse_options(Some(GridKind::Mgrs)),
    ]);
}

/// Grid codes parsed from different spellings of one cell (or square)
/// compare, and so hash, equal.
#[test]
fn grid_codes_hash_consistently() {
    let geohash = |s| Geohash::parse(s).unwrap();
    assert_hash_consistent(&[geohash("u4pruy"), geohash("U4PRUY"), geohash("u4pruz")]);
    let plus = |s| PlusCode::parse(s).unwrap();
    assert_hash_consistent(&[
        plus("8FVC9G8F+6X"),
        plus("8fvc9g8f+6x"),
        plus("8FVC9G8F+6W"),
    ]);
    // `Mgrs` implements `Hash` by hand, over its canonical string.
    let mgrs = |s| Mgrs::parse(s).unwrap();
    assert_hash_consistent(&[
        mgrs("4QFJ12345678"),
        mgrs("04qfj 1234 5678"),
        mgrs("04QFJ12345679"),
    ]);
    let maidenhead = |s| Maidenhead::parse(s).unwrap();
    assert_hash_consistent(&[
        maidenhead("JN58td"),
        maidenhead("jn58TD"),
        maidenhead("JN58te"),
    ]);
}

/// The feature-gated types deriving `Hash`.
#[test]
fn feature_gated_types_hash_consistently() {
    #[cfg(feature = "h3")]
    {
        use crate::dgg::H3Cell;
        let at = |lat| H3Cell::encode(Coordinate::wgs84(lat, -74.0).unwrap(), 9).unwrap();
        assert_hash_consistent(&[at(40.7), at(40.7), at(41.7)]);
    }
    #[cfg(feature = "s2")]
    {
        use crate::dgg::S2CellId;
        let at = |lat| S2CellId::encode(Coordinate::wgs84(lat, -74.0).unwrap(), 12).unwrap();
        assert_hash_consistent(&[at(40.7), at(40.7), at(41.7)]);
    }
    #[cfg(feature = "exif")]
    {
        use crate::exif::{ExifOptions, ExifRational, NorthReference, NullIslandPolicy};
        assert_hash_consistent(&[
            ExifRational::Unsigned(1, 2),
            ExifRational::Signed(-1, 2),
            ExifRational::Unsigned(1, 2),
            ExifRational::Signed(-1, 2),
        ]);
        let north = [NorthReference::True, NorthReference::Magnetic];
        assert_hash_consistent(&[north, north].concat());
        let policies = [NullIslandPolicy::Accept, NullIslandPolicy::Reject];
        assert_hash_consistent(&[policies, policies].concat());
        let options = |p| ExifOptions::default().with_null_island(p);
        assert_hash_consistent(&[
            options(NullIslandPolicy::Accept),
            options(NullIslandPolicy::Reject),
            options(NullIslandPolicy::Accept),
        ]);
    }
    #[cfg(feature = "geotag")]
    {
        use crate::geotag::Interpolation;
        let modes = [Interpolation::Geodesic, Interpolation::Nearest];
        assert_hash_consistent(&[modes, modes].concat());
    }
    #[cfg(feature = "map")]
    {
        use crate::map::{DisplayDatum, Tile};
        let datums = [DisplayDatum::Wgs84, DisplayDatum::Gcj02];
        assert_hash_consistent(&[datums, datums].concat());
        let tile = |z, x, y| Tile::new(z, x, y).unwrap();
        assert_hash_consistent(&[
            tile(3, 3, 2),
            Tile::from_quadkey("031").unwrap(),
            tile(3, 3, 3),
        ]);
    }
    #[cfg(feature = "privacy")]
    {
        use crate::privacy::{Precision, SnapGrid, ZoneAction};
        let grids = [
            SnapGrid::Geohash,
            #[cfg(feature = "h3")]
            SnapGrid::H3,
            #[cfg(feature = "s2")]
            SnapGrid::S2,
        ];
        assert_hash_consistent(&[grids, grids].concat());
        let precision = |level| Precision::new(SnapGrid::Geohash, level).unwrap();
        assert_hash_consistent(&[precision(5), precision(6), precision(5)]);
        assert_hash_consistent(&[
            ZoneAction::Strip,
            ZoneAction::Coarsen(precision(5)),
            ZoneAction::Coarsen(precision(6)),
            ZoneAction::Strip,
            ZoneAction::Coarsen(precision(5)),
        ]);
    }
    #[cfg(feature = "reverse")]
    {
        use crate::reverse::FeatureClass;
        let classes = [
            FeatureClass::Section,
            FeatureClass::Locality,
            FeatureClass::Admin4Seat,
            FeatureClass::Admin3Seat,
            FeatureClass::Admin2Seat,
            FeatureClass::Admin1Seat,
            FeatureClass::Capital,
        ];
        assert_hash_consistent(&[classes, classes].concat());
    }
}
