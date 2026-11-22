// Java access to the grid exports and records with unsigned values (see Unsigned.kt).
package io.github.justin13888.geocoordinates

import uniffi.geocoordinates_ffi.Coordinate
import uniffi.geocoordinates_ffi.GeoException
import uniffi.geocoordinates_ffi.MgrsPrecision
import uniffi.geocoordinates_ffi.Utm
import uniffi.geocoordinates_ffi.UtmHemisphere
import uniffi.geocoordinates_ffi.geohashEncode as ffiGeohashEncode
import uniffi.geocoordinates_ffi.maidenheadEncode as ffiMaidenheadEncode
import uniffi.geocoordinates_ffi.mgrsPrecisionMeters as ffiMgrsPrecisionMeters
import uniffi.geocoordinates_ffi.plusCodeEncode as ffiPlusCodeEncode
import uniffi.geocoordinates_ffi.utmCentralMeridianDeg as ffiUtmCentralMeridianDeg
import uniffi.geocoordinates_ffi.utmZoneFor as ffiUtmZoneFor

/** Java access to the Plus Code, geohash, Maidenhead, UTM and MGRS exports and [Utm]. */
object Grids {
    /** `plus_code_encode`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun plusCodeEncode(coord: Coordinate, length: Int): String = ffiPlusCodeEncode(coord, toUByteArg(length, "length"))

    /** `geohash_encode`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun geohashEncode(coord: Coordinate, length: Int): String = ffiGeohashEncode(coord, toUByteArg(length, "length"))

    /** `maidenhead_encode`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun maidenheadEncode(coord: Coordinate, pairs: Int): String = ffiMaidenheadEncode(coord, toUByteArg(pairs, "pairs"))

    /** `utm_zone_for`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun utmZoneFor(coord: Coordinate): Int = ffiUtmZoneFor(coord).toInt()

    /** `utm_central_meridian_deg`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun utmCentralMeridianDeg(zone: Int): Double = ffiUtmCentralMeridianDeg(toUByteArg(zone, "zone"))

    /** `mgrs_precision_meters`. */
    @JvmStatic
    fun mgrsPrecisionMeters(precision: MgrsPrecision): Long = ffiMgrsPrecisionMeters(precision).toLong()

    /** A [Utm] record (unchecked, like the record; see `utmValidate`). */
    @JvmStatic
    fun utm(zone: Int, hemisphere: UtmHemisphere, eastingM: Double, northingM: Double): Utm =
        Utm(toUByteArg(zone, "zone"), hemisphere, eastingM, northingM)

    /** [Utm.zone]. */
    @JvmStatic
    fun utmZone(utm: Utm): Int = utm.zone.toInt()
}
