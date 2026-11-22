// Java access to the reverse-geocoding, EXIF, privacy and error records with
// unsigned fields (see Unsigned.kt).
package io.github.justin13888.geocoordinates

import uniffi.geocoordinates_ffi.ExifGps
import uniffi.geocoordinates_ffi.ExifRational
import uniffi.geocoordinates_ffi.FeatureClass
import uniffi.geocoordinates_ffi.GeoException
import uniffi.geocoordinates_ffi.Place
import uniffi.geocoordinates_ffi.Precision
import uniffi.geocoordinates_ffi.ReverseOptions
import uniffi.geocoordinates_ffi.SnapGrid

/** Java access to [Place.population] and [ReverseOptions.minPopulation], which are `u64`. */
object Places {
    /** [Place.population], or null where unknown. Build the rest of a place with `placeNew`. */
    @JvmStatic
    fun placePopulation(place: Place): Long? = place.population?.toLong()

    /** Sets [Place.population]; null for unknown. */
    @JvmStatic
    fun setPlacePopulation(place: Place, population: Long?) {
        place.population = population?.toULong()
    }

    /** A [ReverseOptions] record (`reverseOptionsDefault` gives the defaults). */
    @JvmStatic
    fun reverseOptions(
        languages: List<String>,
        maxDistanceM: Double,
        minPopulation: Long,
        minFeatureClass: FeatureClass?,
    ): ReverseOptions = ReverseOptions(languages, maxDistanceM, minPopulation.toULong(), minFeatureClass)

    /** [ReverseOptions.minPopulation]. */
    @JvmStatic
    fun reverseOptionsMinPopulation(options: ReverseOptions): Long = options.minPopulation.toLong()

    /** Sets [ReverseOptions.minPopulation]. */
    @JvmStatic
    fun setReverseOptionsMinPopulation(options: ReverseOptions, minPopulation: Long) {
        options.minPopulation = minPopulation.toULong()
    }
}

/** Java access to [ExifGps] and [ExifRational.Unsigned], whose fields are unsigned. */
object Exif {
    /** An [ExifGps] with every tag unset; set each tag through its setter. */
    @JvmStatic
    fun exifGps(): ExifGps = ExifGps()

    /** [ExifGps.altitudeRef], or null where unset. */
    @JvmStatic
    fun exifGpsAltitudeRef(gps: ExifGps): Int? = gps.altitudeRef?.toInt()

    /** Sets [ExifGps.altitudeRef], in `[0, 255]`; null to unset it. */
    @JvmStatic
    fun setExifGpsAltitudeRef(gps: ExifGps, altitudeRef: Int?) {
        gps.altitudeRef = toOptionalUByteArg(altitudeRef, "altitudeRef")
    }

    /** An EXIF `RATIONAL`, each part in `[0, 2^32)`. */
    @JvmStatic
    fun unsignedRational(numerator: Long, denominator: Long): ExifRational.Unsigned =
        ExifRational.Unsigned(toUIntArg(numerator, "numerator"), toUIntArg(denominator, "denominator"))

    /** [ExifRational.Unsigned.numerator]. */
    @JvmStatic
    fun unsignedRationalNumerator(rational: ExifRational.Unsigned): Long = rational.numerator.toLong()

    /** [ExifRational.Unsigned.denominator]. */
    @JvmStatic
    fun unsignedRationalDenominator(rational: ExifRational.Unsigned): Long = rational.denominator.toLong()
}

/** Java access to [Precision], whose level is unsigned. */
object Privacy {
    /** A [Precision] record (unchecked, like the record; see `precisionValidate`). */
    @JvmStatic
    fun precision(grid: SnapGrid, level: Int): Precision = Precision(grid, toUByteArg(level, "level"))

    /** [Precision.level]. */
    @JvmStatic
    fun precisionLevel(precision: Precision): Int = precision.level.toInt()
}

/** Java access to [GeoException.InvalidCellId], whose offending id is `u64`. */
object GeoErrors {
    /** A [GeoException.InvalidCellId], for a foreign `ReverseGeocoder` or a test to throw. */
    @JvmStatic
    fun invalidCellId(grid: String, value: Long): GeoException.InvalidCellId =
        GeoException.InvalidCellId(grid, value.toULong())

    /** [GeoException.InvalidCellId.value]: the offending id's 64 bits. */
    @JvmStatic
    fun invalidCellIdValue(error: GeoException.InvalidCellId): Long = error.value.toLong()
}
