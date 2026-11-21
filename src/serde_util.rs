//! Serde codecs shared by more than one type's wire shape.

/// (De)serializes an `f64` whose `+∞` means "unbounded" (an
/// [`Approx`](crate::Approx) error bound, a sphere's inverse flattening) with
/// `f64::INFINITY` as none, because JSON writes infinity as `null` and cannot
/// read `null` back as a number. A finite value keeps its plain-number wire
/// shape.
pub(crate) mod infinity_as_none {
    use serde::{Deserialize, Deserializer, Serializer};

    // serde's `serialize_with` contract passes the field by reference.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(crate) fn serialize<S: Serializer>(value: &f64, s: S) -> Result<S::Ok, S::Error> {
        if *value == f64::INFINITY {
            s.serialize_none()
        } else {
            s.serialize_some(value)
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::INFINITY))
    }
}
