//! The serde wire shape of a [`Place`], validated through [`Place::from_parts`].

use std::collections::BTreeMap;

use super::{FeatureClass, Place};
use crate::approx::Approx;
use crate::coord::Coordinate;
use crate::error::{Error, Result};
use crate::units::Length;

/// The wire shape of a [`Place`], validated on its way in.
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct PlaceRepr {
    name: String,
    coordinate: Coordinate,
    country_code: String,
    admin1_code: Option<String>,
    admin1_name: Option<String>,
    admin2_code: Option<String>,
    admin2_name: Option<String>,
    feature_class: Option<FeatureClass>,
    population: Option<u64>,
    #[serde(rename = "distance_m")]
    distance: Length,
    query: Approx<Coordinate>,
    #[serde(default)]
    localized_names: BTreeMap<String, String>,
}

impl From<Place> for PlaceRepr {
    fn from(place: Place) -> Self {
        let (admin1_code, admin1_name) = place.admin1.unzip();
        let (admin2_code, admin2_name) = place.admin2.unzip();
        Self {
            name: place.name,
            coordinate: place.coordinate,
            country_code: place.country_code,
            admin1_code,
            admin1_name,
            admin2_code,
            admin2_name,
            feature_class: place.feature_class,
            population: place.population,
            distance: place.distance,
            query: place.query,
            localized_names: place.localized_names,
        }
    }
}

impl TryFrom<PlaceRepr> for Place {
    type Error = Error;

    fn try_from(repr: PlaceRepr) -> Result<Self> {
        Self::from_parts(
            repr.name,
            repr.coordinate,
            &repr.country_code,
            repr.query,
            (repr.admin1_code, repr.admin1_name),
            (repr.admin2_code, repr.admin2_name),
            repr.feature_class,
            repr.population,
            repr.localized_names,
            repr.distance,
        )
    }
}
