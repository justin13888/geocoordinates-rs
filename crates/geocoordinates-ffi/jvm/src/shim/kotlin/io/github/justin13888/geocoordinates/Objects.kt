// Java access to the `SpatialIndex`, `Track` and `Gazetteer` methods and the
// spatial-index record with unsigned values (see Unsigned.kt).
package io.github.justin13888.geocoordinates

import uniffi.geocoordinates_ffi.Coordinate
import uniffi.geocoordinates_ffi.Gazetteer
import uniffi.geocoordinates_ffi.GeoException
import uniffi.geocoordinates_ffi.SpatialIndex
import uniffi.geocoordinates_ffi.SpatialNeighbor
import uniffi.geocoordinates_ffi.Track

/** Java access to [SpatialIndex] and [SpatialNeighbor], whose counts and positions are `u64`. */
object SpatialIndexes {
    /** [SpatialIndex.len]. */
    @JvmStatic
    fun spatialIndexLen(index: SpatialIndex): Long = index.len().toLong()

    /** [SpatialIndex.nearest], with [k] in `[0, 2^32)`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun spatialIndexNearest(index: SpatialIndex, center: Coordinate, k: Long): List<SpatialNeighbor> =
        index.nearest(center, toUIntArg(k, "k"))

    /** A [SpatialNeighbor] record. */
    @JvmStatic
    fun spatialNeighbor(coordinate: Coordinate, index: Long, distanceM: Double): SpatialNeighbor =
        SpatialNeighbor(coordinate, index.toULong(), distanceM)

    /** [SpatialNeighbor.index], the entry's position in the build list. */
    @JvmStatic
    fun spatialNeighborIndex(neighbor: SpatialNeighbor): Long = neighbor.index.toLong()
}

/** Java access to the [Track] counts, which are `u64`. */
object Tracks {
    /** [Track.len]. */
    @JvmStatic
    fun trackLen(track: Track): Long = track.len().toLong()

    /** [Track.droppedUntimed]. */
    @JvmStatic
    fun trackDroppedUntimed(track: Track): Long = track.droppedUntimed().toLong()
}

/** Java access to the [Gazetteer] entry count, which is `u64`. */
object Gazetteers {
    /** [Gazetteer.len]. */
    @JvmStatic
    fun gazetteerLen(gazetteer: Gazetteer): Long = gazetteer.len().toLong()
}
