// Java access to the map tile record and exports (see Unsigned.kt).
package io.github.justin13888.geocoordinates

import uniffi.geocoordinates_ffi.Coordinate
import uniffi.geocoordinates_ffi.GeoBounds
import uniffi.geocoordinates_ffi.GeoException
import uniffi.geocoordinates_ffi.Tile
import uniffi.geocoordinates_ffi.tileFromCoordinate as ffiTileFromCoordinate
import uniffi.geocoordinates_ffi.tileMaxZoom as ffiTileMaxZoom
import uniffi.geocoordinates_ffi.tilesCovering as ffiTilesCovering

/** Java access to [Tile] and the tile exports that take or return an unsigned value. */
object Tiles {
    /** A [Tile] record, with [x] and [y] in `[0, 2^32)` (unchecked, like the record; see `tileValidate`). */
    @JvmStatic
    fun tile(z: Int, x: Long, y: Long): Tile = Tile(toUByteArg(z, "z"), toUIntArg(x, "x"), toUIntArg(y, "y"))

    /** [Tile.z]. */
    @JvmStatic
    fun tileZ(tile: Tile): Int = tile.z.toInt()

    /** [Tile.x]. */
    @JvmStatic
    fun tileX(tile: Tile): Long = tile.x.toLong()

    /** [Tile.y]. */
    @JvmStatic
    fun tileY(tile: Tile): Long = tile.y.toLong()

    /** `tile_max_zoom`. */
    @JvmStatic
    fun tileMaxZoom(): Int = ffiTileMaxZoom().toInt()

    /** `tile_from_coordinate`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun tileFromCoordinate(point: Coordinate, z: Int): Tile = ffiTileFromCoordinate(point, toUByteArg(z, "z"))

    /** `tiles_covering`, with [maxTiles] in `[0, 2^32)`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun tilesCovering(bounds: GeoBounds, z: Int, maxTiles: Long): List<Tile> =
        ffiTilesCovering(bounds, toUByteArg(z, "z"), toUIntArg(maxTiles, "maxTiles"))
}
