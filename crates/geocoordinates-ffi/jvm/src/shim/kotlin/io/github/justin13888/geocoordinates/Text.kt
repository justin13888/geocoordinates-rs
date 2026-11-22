// Java access to the angle and format records with unsigned fields (see Unsigned.kt).
package io.github.justin13888.geocoordinates

import uniffi.geocoordinates_ffi.Ddm
import uniffi.geocoordinates_ffi.Dms
import uniffi.geocoordinates_ffi.Hemisphere
import uniffi.geocoordinates_ffi.HemisphereStyle
import uniffi.geocoordinates_ffi.Representation
import uniffi.geocoordinates_ffi.SymbolStyle

/** Java access to [Dms] and [Ddm], whose whole degrees and minutes are unsigned. */
object Angles {
    /** A [Dms] record (unchecked, like the record; see `dmsValidate`). */
    @JvmStatic
    fun dms(degrees: Int, minutes: Int, seconds: Double, hemisphere: Hemisphere): Dms =
        Dms(toUShortArg(degrees, "degrees"), toUByteArg(minutes, "minutes"), seconds, hemisphere)

    /** [Dms.degrees]. */
    @JvmStatic
    fun dmsDegrees(dms: Dms): Int = dms.degrees.toInt()

    /** [Dms.minutes]. */
    @JvmStatic
    fun dmsMinutes(dms: Dms): Int = dms.minutes.toInt()

    /** A [Ddm] record (unchecked, like the record; see `ddmValidate`). */
    @JvmStatic
    fun ddm(degrees: Int, minutes: Double, hemisphere: Hemisphere): Ddm =
        Ddm(toUShortArg(degrees, "degrees"), minutes, hemisphere)

    /** [Ddm.degrees]. */
    @JvmStatic
    fun ddmDegrees(ddm: Ddm): Int = ddm.degrees.toInt()
}

/** Java access to the [Representation] variants whose settings are unsigned. */
object Formats {
    /** [Representation.DecimalDegrees]; a null [decimals] derives it as the record does. */
    @JvmStatic
    fun decimalDegrees(decimals: Int?, hemisphere: HemisphereStyle): Representation.DecimalDegrees =
        Representation.DecimalDegrees(toOptionalUByteArg(decimals, "decimals"), hemisphere)

    /** [Representation.DecimalDegrees.decimals]. */
    @JvmStatic
    fun decimalDegreesDecimals(representation: Representation.DecimalDegrees): Int? =
        representation.decimals?.toInt()

    /** [Representation.Dms]; a null [secondDecimals] derives it as the record does. */
    @JvmStatic
    fun dms(secondDecimals: Int?, symbols: SymbolStyle, hemisphere: HemisphereStyle): Representation.Dms =
        Representation.Dms(toOptionalUByteArg(secondDecimals, "secondDecimals"), symbols, hemisphere)

    /** [Representation.Dms.secondDecimals]. */
    @JvmStatic
    fun dmsSecondDecimals(representation: Representation.Dms): Int? = representation.secondDecimals?.toInt()

    /** [Representation.Ddm]; a null [minuteDecimals] derives it as the record does. */
    @JvmStatic
    fun ddm(minuteDecimals: Int?, symbols: SymbolStyle, hemisphere: HemisphereStyle): Representation.Ddm =
        Representation.Ddm(toOptionalUByteArg(minuteDecimals, "minuteDecimals"), symbols, hemisphere)

    /** [Representation.Ddm.minuteDecimals]. */
    @JvmStatic
    fun ddmMinuteDecimals(representation: Representation.Ddm): Int? = representation.minuteDecimals?.toInt()

    /** [Representation.PlusCode]. */
    @JvmStatic
    fun plusCode(length: Int): Representation.PlusCode = Representation.PlusCode(toUByteArg(length, "length"))

    /** [Representation.PlusCode.length]. */
    @JvmStatic
    fun plusCodeLength(representation: Representation.PlusCode): Int = representation.length.toInt()

    /** [Representation.Geohash]. */
    @JvmStatic
    fun geohash(length: Int): Representation.Geohash = Representation.Geohash(toUByteArg(length, "length"))

    /** [Representation.Geohash.length]. */
    @JvmStatic
    fun geohashLength(representation: Representation.Geohash): Int = representation.length.toInt()

    /** [Representation.Maidenhead]. */
    @JvmStatic
    fun maidenhead(pairs: Int): Representation.Maidenhead = Representation.Maidenhead(toUByteArg(pairs, "pairs"))

    /** [Representation.Maidenhead.pairs]. */
    @JvmStatic
    fun maidenheadPairs(representation: Representation.Maidenhead): Int = representation.pairs.toInt()
}
