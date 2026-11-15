//! The string conversions of [`Mgrs`]: parsing sugar over [`Mgrs::parse`], and
//! the canonical string out through `String`, `AsRef<str>` and `Display`.

use core::str::FromStr;

use super::Mgrs;
use crate::error::Result;

impl TryFrom<&str> for Mgrs {
    type Error = crate::Error;

    /// Validate as [`Mgrs::parse`] does.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for a malformed reference.
    fn try_from(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl TryFrom<String> for Mgrs {
    type Error = crate::Error;

    /// Validate as [`Mgrs::parse`] does; this is the serde route.
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for a malformed reference.
    fn try_from(s: String) -> Result<Self> {
        Self::parse(&s)
    }
}

impl FromStr for Mgrs {
    type Err = crate::Error;

    /// Equivalent to [`Mgrs::parse`].
    ///
    /// # Errors
    /// Returns [`crate::Error::InvalidGridRef`] for a malformed reference.
    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl From<Mgrs> for String {
    /// The canonical string.
    fn from(mgrs: Mgrs) -> String {
        mgrs.text
    }
}

impl AsRef<str> for Mgrs {
    /// The canonical string, as [`Mgrs::as_str`] returns and `Display` writes.
    ///
    /// ```
    /// use geocoordinates::grids::Mgrs;
    ///
    /// let mgrs = Mgrs::try_from("4qfj12345678")?;
    /// assert_eq!(mgrs.as_ref(), "04QFJ12345678");
    /// assert_eq!(mgrs.as_ref(), mgrs.to_string());
    /// # Ok::<(), geocoordinates::Error>(())
    /// ```
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl core::fmt::Display for Mgrs {
    /// Writes the canonical string, which parses back to an equal value.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.text)
    }
}
