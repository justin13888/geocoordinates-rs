//! Full-surface, self-verifying field lab for `geocoordinates`.
//!
//! Run with:
//!
//! ```text
//! cargo run --all-features --example flagship
//! ```
//!
//! Every section uses only public API. Assertions make this executable
//! documentation fail loudly if a reference value, round trip, error bound,
//! axis order, or typed failure regresses.
//!
//! Each capability group lives in its own module; `support` holds the shared
//! assertion helpers and reference values.

mod boundaries;
mod china;
mod encoded_grids;
mod formatting;
mod geodesics;
mod interchange;
mod primitives;
mod projected_grids;
mod support;

use std::error::Error as StdError;

use boundaries::boundaries_and_typed_errors;
use china::china_datums_and_dispatch;
use encoded_grids::encoded_and_global_grids;
use formatting::formatting_and_parsing;
use geodesics::geodesics_and_local_frames;
use interchange::interchange_and_nmea;
use primitives::primitives_and_serde;
use projected_grids::classic_datums_and_projected_grids;

#[cfg(not(test))]
fn main() -> Result<(), Box<dyn StdError>> {
    println!(
        "geocoordinates {} — full-surface field lab",
        env!("CARGO_PKG_VERSION")
    );
    for section in [
        primitives_and_serde()?,
        formatting_and_parsing()?,
        interchange_and_nmea()?,
        china_datums_and_dispatch()?,
        geodesics_and_local_frames()?,
        classic_datums_and_projected_grids()?,
        encoded_and_global_grids()?,
        boundaries_and_typed_errors()?,
    ] {
        println!("[ok] {section}");
    }
    println!("All 8 shipped Rust capability groups passed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn StdError>>;

    #[test]
    fn primitives_and_serde_work() -> TestResult {
        primitives_and_serde().map(|_| ())
    }

    #[test]
    fn formatting_and_parsing_work() -> TestResult {
        formatting_and_parsing().map(|_| ())
    }

    #[test]
    fn interchange_and_nmea_work() -> TestResult {
        interchange_and_nmea().map(|_| ())
    }

    #[test]
    fn china_datums_and_dispatch_work() -> TestResult {
        china_datums_and_dispatch().map(|_| ())
    }

    #[test]
    fn geodesics_and_local_frames_work() -> TestResult {
        geodesics_and_local_frames().map(|_| ())
    }

    #[test]
    fn classic_datums_and_projected_grids_work() -> TestResult {
        classic_datums_and_projected_grids().map(|_| ())
    }

    #[test]
    fn encoded_and_global_grids_work() -> TestResult {
        encoded_and_global_grids().map(|_| ())
    }

    #[test]
    fn boundaries_and_typed_errors_work() -> TestResult {
        boundaries_and_typed_errors().map(|_| ())
    }
}
