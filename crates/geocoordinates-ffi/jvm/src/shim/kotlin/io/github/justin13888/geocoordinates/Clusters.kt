// Java access to the cluster records, exports and `ClusterIndex` (see Unsigned.kt).
package io.github.justin13888.geocoordinates

import java.time.Instant
import uniffi.geocoordinates_ffi.Cluster
import uniffi.geocoordinates_ffi.ClusterIndex
import uniffi.geocoordinates_ffi.Coordinate
import uniffi.geocoordinates_ffi.DbscanResult
import uniffi.geocoordinates_ffi.GeoBounds
import uniffi.geocoordinates_ffi.GeoException
import uniffi.geocoordinates_ffi.GridBucket
import uniffi.geocoordinates_ffi.MapCluster
import uniffi.geocoordinates_ffi.PointLabel
import uniffi.geocoordinates_ffi.Trip
import uniffi.geocoordinates_ffi.bucketByGeohash as ffiBucketByGeohash
import uniffi.geocoordinates_ffi.bucketByH3 as ffiBucketByH3
import uniffi.geocoordinates_ffi.clusterMaxZoom as ffiClusterMaxZoom
import uniffi.geocoordinates_ffi.dbscan as ffiDbscan

/**
 * Java access to the cluster records ([Cluster], [MapCluster], [Trip],
 * [PointLabel.Cluster]), the cluster exports, and the [ClusterIndex] methods
 * that take or return an unsigned value. Ids, counts and offsets are `u64`.
 */
object Clusters {
    /** A [Cluster] record (unchecked, like the record; see `clusterValidate`). */
    @JvmStatic
    fun cluster(
        id: Long,
        centroid: Coordinate,
        centroidMaxErrorM: Double,
        bounds: GeoBounds,
        count: Long,
        members: LongArray,
    ): Cluster = Cluster(id.toULong(), centroid, centroidMaxErrorM, bounds, count.toULong(), members.toULongList())

    /** [Cluster.id]. */
    @JvmStatic
    fun clusterId(cluster: Cluster): Long = cluster.id.toLong()

    /** [Cluster.count]. */
    @JvmStatic
    fun clusterCount(cluster: Cluster): Long = cluster.count.toLong()

    /** [Cluster.members], the members' positions in the input list. */
    @JvmStatic
    fun clusterMembers(cluster: Cluster): LongArray = cluster.members.toLongArray()

    /** A [MapCluster] record. */
    @JvmStatic
    fun mapCluster(
        id: Long,
        zoom: Int,
        centroid: Coordinate,
        centroidMaxErrorM: Double,
        bounds: GeoBounds,
        count: Long,
    ): MapCluster =
        MapCluster(id.toULong(), toUByteArg(zoom, "zoom"), centroid, centroidMaxErrorM, bounds, count.toULong())

    /** [MapCluster.id]. */
    @JvmStatic
    fun mapClusterId(cluster: MapCluster): Long = cluster.id.toLong()

    /** [MapCluster.zoom]. */
    @JvmStatic
    fun mapClusterZoom(cluster: MapCluster): Int = cluster.zoom.toInt()

    /** [MapCluster.count]. */
    @JvmStatic
    fun mapClusterCount(cluster: MapCluster): Long = cluster.count.toLong()

    /** A [Trip] record. */
    @JvmStatic
    fun trip(visits: LongArray, start: Instant, end: Instant, bounds: GeoBounds): Trip =
        Trip(visits.toULongList(), start, end, bounds)

    /** [Trip.visits], the visits' positions in the input list. */
    @JvmStatic
    fun tripVisits(trip: Trip): LongArray = trip.visits.toLongArray()

    /** A [PointLabel.Cluster] label. */
    @JvmStatic
    fun pointLabelCluster(id: Long): PointLabel.Cluster = PointLabel.Cluster(id.toULong())

    /** [PointLabel.Cluster.id]. */
    @JvmStatic
    fun pointLabelClusterId(label: PointLabel.Cluster): Long = label.id.toLong()

    /** `dbscan`, with [minPoints] in `[0, 2^32)`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun dbscan(points: List<Coordinate>, epsM: Double, minPoints: Long): DbscanResult =
        ffiDbscan(points, epsM, toUIntArg(minPoints, "minPoints"))

    /** `bucket_by_geohash`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun bucketByGeohash(points: List<Coordinate>, length: Int): List<GridBucket> =
        ffiBucketByGeohash(points, toUByteArg(length, "length"))

    /** `bucket_by_h3`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun bucketByH3(points: List<Coordinate>, resolution: Int): List<GridBucket> =
        ffiBucketByH3(points, toUByteArg(resolution, "resolution"))

    /** `cluster_max_zoom`. */
    @JvmStatic
    fun clusterMaxZoom(): Int = ffiClusterMaxZoom().toInt()

    /** [ClusterIndex.len]. */
    @JvmStatic
    fun clusterIndexLen(index: ClusterIndex): Long = index.len().toLong()

    /** [ClusterIndex.coordinate]: the point with [pointId], or null. */
    @JvmStatic
    fun clusterIndexCoordinate(index: ClusterIndex, pointId: Long): Coordinate? = index.coordinate(pointId.toULong())

    /** [ClusterIndex.insert]: the new point's id. */
    @JvmStatic
    @Throws(GeoException::class)
    fun clusterIndexInsert(index: ClusterIndex, point: Coordinate): Long = index.insert(point).toLong()

    /** [ClusterIndex.remove]. */
    @JvmStatic
    @Throws(GeoException::class)
    fun clusterIndexRemove(index: ClusterIndex, pointId: Long): Coordinate = index.remove(pointId.toULong())

    /** [ClusterIndex.clusters]. */
    @JvmStatic
    @Throws(GeoException::class)
    fun clusterIndexClusters(index: ClusterIndex, bounds: GeoBounds, zoom: Int): List<MapCluster> =
        index.clusters(bounds, toUByteArg(zoom, "zoom"))

    /** [ClusterIndex.children]. */
    @JvmStatic
    @Throws(GeoException::class)
    fun clusterIndexChildren(index: ClusterIndex, clusterId: Long): List<MapCluster> =
        index.children(clusterId.toULong())

    /** [ClusterIndex.leaves]: point ids, with [limit] in `[0, 2^32)`. */
    @JvmStatic
    @Throws(GeoException::class)
    fun clusterIndexLeaves(index: ClusterIndex, clusterId: Long, limit: Long, offset: Long): LongArray =
        index.leaves(clusterId.toULong(), toUIntArg(limit, "limit"), offset.toULong()).toLongArray()
}
