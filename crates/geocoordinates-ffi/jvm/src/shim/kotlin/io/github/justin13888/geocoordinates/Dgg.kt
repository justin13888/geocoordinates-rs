// Java access to the H3 and S2 cell records and exports (see Unsigned.kt).
package io.github.justin13888.geocoordinates

import uniffi.geocoordinates_ffi.Coordinate
import uniffi.geocoordinates_ffi.GeoException
import uniffi.geocoordinates_ffi.H3Cell
import uniffi.geocoordinates_ffi.S2CellId
import uniffi.geocoordinates_ffi.h3Encode as ffiH3Encode
import uniffi.geocoordinates_ffi.h3Resolution as ffiH3Resolution
import uniffi.geocoordinates_ffi.s2Encode as ffiS2Encode
import uniffi.geocoordinates_ffi.s2Level as ffiS2Level

/** Java access to [S2CellId] and the `s2_*` exports that take or return an unsigned value. */
object S2Cells {
    /** An [S2CellId] holding the 64 bits of [value] (unchecked, like the record; see `s2Validate`). */
    @JvmStatic
    fun fromLong(value: Long): S2CellId = S2CellId(value.toULong())

    /** The cell id's 64 bits; an id above 2^63 - 1 reads as negative. */
    @JvmStatic
    fun toLong(cell: S2CellId): Long = cell.value.toLong()

    /** `s2_encode`, with [level] in `[0, 255]` (the export accepts 0 through 30). */
    @JvmStatic
    @Throws(GeoException::class)
    fun s2Encode(coord: Coordinate, level: Int): S2CellId = ffiS2Encode(coord, toUByteArg(level, "level"))

    /** `s2_level`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun s2Level(cell: S2CellId): Int = ffiS2Level(cell).toInt()
}

/** Java access to [H3Cell] and the `h3_*` exports that take or return an unsigned value. */
object H3Cells {
    /** An [H3Cell] holding the 64 bits of [value] (unchecked, like the record; see `h3Validate`). */
    @JvmStatic
    fun fromLong(value: Long): H3Cell = H3Cell(value.toULong())

    /** The cell index's 64 bits. */
    @JvmStatic
    fun toLong(cell: H3Cell): Long = cell.value.toLong()

    /** `h3_encode`, with [resolution] in `[0, 255]` (the export accepts 0 through 15). */
    @JvmStatic
    @Throws(GeoException::class)
    fun h3Encode(coord: Coordinate, resolution: Int): H3Cell =
        ffiH3Encode(coord, toUByteArg(resolution, "resolution"))

    /** `h3_resolution`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun h3Resolution(cell: H3Cell): Int = ffiH3Resolution(cell).toInt()
}
