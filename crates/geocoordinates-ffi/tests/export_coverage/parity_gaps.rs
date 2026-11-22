//! The core items `every_core_item_has_an_ffi_form` cannot match by name
//! alone. Both lists may only shrink: an entry whose core item an FFI form
//! now reaches by name, or that names no core item, fails the test until it is
//! removed, and a `RENAMED` target that names no export or record field fails
//! it too. A new core item goes in neither list without review: give it an FFI
//! form under the naming scheme instead.

/// Core items whose FFI form has another name (the `src/lib.rs` idiom table):
/// `(core item, FFI export)` or `(core item, FFI Record.field)`.
pub const RENAMED: &[(&str, &str)] = &[
    ("CaptureGeo::from_exif", "parse_exif"),
    ("CaptureGeo::from_exif_with", "parse_exif_with"),
    ("CaptureGeo::from_xmp", "parse_xmp"),
    ("CaptureGeo::from_xmp_with", "parse_xmp_with"),
    // The named constructors are `coordinate_new` with the datum as an argument.
    ("Coordinate::bd09", "coordinate_new"),
    ("Coordinate::gcj02", "coordinate_new"),
    ("Coordinate::wgs84", "coordinate_new"),
    ("Coordinate::try_to_crs", "convert"),
    ("Dbscan::clusters", "DbscanResult.clusters"),
    ("Dbscan::labels", "DbscanResult.labels"),
    ("ExifOptions::new", "exif_options_default"),
    // `ExclusionZone` is a data enum checked by `exclusion_zone_validate`.
    ("ExclusionZone::from_bounds", "exclusion_zone_validate"),
    ("ExclusionZone::from_circle", "exclusion_zone_validate"),
    // A record of edges, built from its corners or its edges, checked by
    // `geo_bounds_validate`.
    ("GeoBounds::from_corners", "geo_bounds_validate"),
    ("GeoBounds::from_edges", "geo_bounds_validate"),
    // A grid code is a `String`; `parse` is its canonical form.
    ("Geohash::parse", "geohash_canonicalize"),
    ("Maidenhead::parse", "maidenhead_canonicalize"),
    ("Mgrs::parse", "mgrs_canonicalize"),
    ("PlusCode::parse", "plus_code_canonicalize"),
    ("Height::length", "height_meters"),
    ("MAX_LATITUDE", "web_mercator_max_latitude_deg"),
    ("MAX_TILE_ZOOM", "tile_max_zoom"),
    ("MAX_ZOOM", "cluster_max_zoom"),
    ("MgrsPrecision::side", "mgrs_precision_meters"),
    ("Neighbor::coordinate", "SpatialNeighbor.coordinate"),
    ("Neighbor::distance", "SpatialNeighbor.distance_m"),
    ("Neighbor::index", "SpatialNeighbor.index"),
    ("NorthBearing::degrees", "NorthBearing.bearing_deg"),
    ("ParseOptions::for_format", "format_options_parse_options"),
    // The `Place` record is every part at once, checked by `place_validate`.
    ("Place::from_parts", "place_validate"),
    ("Place::with_admin1", "Place.admin1_code"),
    ("Place::with_admin2", "Place.admin2_code"),
    ("Place::with_localized_name", "Place.localized_names"),
    // A record `{ grid, level }`, checked by `precision_validate`.
    ("Precision::geohash", "precision_validate"),
    ("Precision::h3", "precision_validate"),
    ("Precision::s2", "precision_validate"),
    ("ReverseOptions::new", "reverse_options_default"),
    ("apply", "apply_exclusion_zones"),
    ("clusters_to_geojson", "clusters_in_view_geojson"),
    ("format", "format_coordinate"),
];

/// Core items with no FFI form.
pub const ALLOWED: &[&str] = &[
    // Deliberate: `Approx<T>` is generic; the FFI returns flat `Approx*` records
    // whose fields are the value and `max_error_m`, and `Approx::new` is
    // Rust-only (`src/lib.rs`).
    "Approx::into_inner",
    "Approx::map",
    "Approx::max_error",
    "Approx::new",
    "Approx::value",
    // Deliberate: `Length` is `f64` meters across the boundary.
    "Length::ZERO",
    "Length::from_meters",
    "Length::meters",
    // Deliberate: a grid code is a `String` and a cell id a `u64` across the
    // boundary, so there is nothing to unwrap.
    "Geohash::as_str",
    "H3Cell::as_u64",
    "Maidenhead::as_str",
    "Mgrs::as_str",
    "PlusCode::as_str",
    "S2CellId::as_u64",
    // Deliberate: `ExclusionZone` is a data enum, so its variant is the getter.
    "ExclusionZone::bounds",
    "ExclusionZone::circle",
    "ExclusionZone::crs",
    // Deliberate: the bindings do not compile the bundled dataset in.
    "Gazetteer::bundled",
    // Deliberate: the FFI index holds coordinates, and its results carry each
    // entry's position for the caller's own payload lookup.
    "Neighbor::value",
    // Deliberate: `NorthBearing` is output-only across the boundary.
    "NorthBearing::new",
];
