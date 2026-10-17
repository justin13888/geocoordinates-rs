package io.github.justin13888.geocoordinates

import java.time.Instant
import kotlin.math.sqrt
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows
import uniffi.geocoordinates_ffi.Accuracy
import uniffi.geocoordinates_ffi.AccuracyKind
import uniffi.geocoordinates_ffi.Bd09
import uniffi.geocoordinates_ffi.Crs
import uniffi.geocoordinates_ffi.Gcj02
import uniffi.geocoordinates_ffi.GeoException
import uniffi.geocoordinates_ffi.H3Cell
import uniffi.geocoordinates_ffi.MgrsPrecision
import uniffi.geocoordinates_ffi.ParseErrorKind
import uniffi.geocoordinates_ffi.UtmHemisphere
import uniffi.geocoordinates_ffi.Wgs84
import uniffi.geocoordinates_ffi.accuracyNew
import uniffi.geocoordinates_ffi.baiduMercatorFromBd09
import uniffi.geocoordinates_ffi.bd09ToWgs84Fast
import uniffi.geocoordinates_ffi.baiduMercatorFromCoordinate
import uniffi.geocoordinates_ffi.confidenceNew
import uniffi.geocoordinates_ffi.convert
import uniffi.geocoordinates_ffi.coordinateNew
import uniffi.geocoordinates_ffi.datumTransformCatalogToWgs84
import uniffi.geocoordinates_ffi.fixFromCoord
import uniffi.geocoordinates_ffi.formatFix
import uniffi.geocoordinates_ffi.formatOptionsDefault
import uniffi.geocoordinates_ffi.fromNmeaSentence
import uniffi.geocoordinates_ffi.gcj02ToBd09
import uniffi.geocoordinates_ffi.gcj02ToWgs84Fast
import uniffi.geocoordinates_ffi.gcj02ToWgs84Refined
import uniffi.geocoordinates_ffi.geodesicDistanceM
import uniffi.geocoordinates_ffi.h3Decode
import uniffi.geocoordinates_ffi.h3Encode
import uniffi.geocoordinates_ffi.mgrsFromCoordinate
import uniffi.geocoordinates_ffi.mgrsToCoordinate
import uniffi.geocoordinates_ffi.outOfChina
import uniffi.geocoordinates_ffi.parseCoordinate
import uniffi.geocoordinates_ffi.utmCentralMeridianDeg
import uniffi.geocoordinates_ffi.utmFromCoordinate
import uniffi.geocoordinates_ffi.wgs84ToBd09
import uniffi.geocoordinates_ffi.wgs84ToGcj02

// Reference triple from src/test_support.rs (coordtransform-rs), lat first; the
// same values crates/geocoordinates-ffi/tests/smoke.py pins.
private val WGS = Wgs84(39.915, 116.404)
private val GCJ = Gcj02(39.91640428150164, 116.41024449916938)
private val BD = Bd09(39.922699552216216, 116.41662724378733)

private fun wgs84(lat: Double, lon: Double) = coordinateNew(lat, lon, Crs.WGS84)

// Exhaustive over the sealed hierarchy: a new `GeoException` variant fails this
// file's compilation until the tests below account for it.
private fun variantOf(e: GeoException): String =
    when (e) {
        is GeoException.OutOfRange -> "OutOfRange"
        is GeoException.OutsideDomain -> "OutsideDomain"
        is GeoException.InvalidValue -> "InvalidValue"
        is GeoException.InvalidCellId -> "InvalidCellId"
        is GeoException.Parse -> "Parse"
        is GeoException.CrsMismatch -> "CrsMismatch"
        is GeoException.InvalidGridRef -> "InvalidGridRef"
        is GeoException.Other -> "Other"
    }

// Asserts `block` throws exactly the variant `T` and returns it, so the test
// reads the variant's typed fields rather than its message.
private inline fun <reified T : GeoException> throwsVariant(noinline block: () -> Unit): T {
    val e = assertThrows<GeoException>(block)
    assertTrue(e is T, "expected ${T::class.simpleName}, got ${variantOf(e)}: ${e.message}")
    return e as T
}

/** Behavioural tests of the Kotlin bindings: conversions, bounds, typed errors. */
class BindingsTest {
    @Test
    fun chinaForwardConversionsAreExact() {
        val gcj = wgs84ToGcj02(WGS)
        assertEquals(GCJ.lat, gcj.lat, 1e-9)
        assertEquals(GCJ.lon, gcj.lon, 1e-9)
        val bd = wgs84ToBd09(WGS)
        assertEquals(BD.lat, bd.lat, 1e-9)
        assertEquals(BD.lon, bd.lon, 1e-9)
        assertTrue(outOfChina(51.5074, -0.1278))
        assertFalse(outOfChina(WGS.lat, WGS.lon))
    }

    @Test
    fun refinedInverseCarriesItsBound() {
        val back = gcj02ToWgs84Refined(GCJ)
        assertEquals(WGS.lat, back.lat, 1e-5)
        assertEquals(WGS.lon, back.lon, 1e-5)
        assertTrue(back.maxErrorM > 0.0 && back.maxErrorM <= 0.5, "bound ${back.maxErrorM}")
    }

    @Test
    fun fastInversesMeasureTheirBounds() {
        val fast = bd09ToWgs84Fast(BD)
        assertEquals(WGS.lat, fast.lat, 1e-4)
        assertEquals(WGS.lon, fast.lon, 1e-4)
        assertEquals(6.5, fast.maxErrorM)
        val edge = gcj02ToWgs84Fast(Gcj02(40.0, 72.006))
        assertTrue(edge.maxErrorM > 300.0, "bound ${edge.maxErrorM}")
    }

    @Test
    fun convertReportsExactAndApproximateLegs() {
        val fwd = convert(wgs84(WGS.lat, WGS.lon), Crs.GCJ02)
        assertEquals(0.0, fwd.maxErrorM)
        assertEquals(Crs.GCJ02, fwd.coord.crs)
        assertEquals(GCJ.lat, fwd.coord.lat, 1e-9)

        val inv = convert(coordinateNew(GCJ.lat, GCJ.lon, Crs.GCJ02), Crs.WGS84)
        assertEquals(0.5, inv.maxErrorM)
        assertEquals(Crs.WGS84, inv.coord.crs)
        assertEquals(WGS.lat, inv.coord.lat, 1e-9)

        // A Helmert leg reports its published TR8350.2 accuracy; Pulkovo has none.
        val nad27 = coordinateNew(40.0, -100.0, Crs.NAD27)
        assertEquals(sqrt(86.0), convert(nad27, Crs.WGS84).maxErrorM, 1e-12)
        assertNull(datumTransformCatalogToWgs84(Crs.PULKOVO42)?.accuracyM)
    }

    @Test
    fun gridsProjectionsAndGeodesics() {
        val code = mgrsFromCoordinate(wgs84(40.0, -75.0), MgrsPrecision.M1)
        assertEquals("18TWK0000027757", code)
        val decoded = mgrsToCoordinate(code)
        assertEquals(sqrt(0.5) / 0.9996, decoded.maxErrorM, 1e-12)
        assertEquals(40.0, decoded.coord.lat, 1e-3)

        val utm = utmFromCoordinate(wgs84(40.0, -75.0))
        assertEquals(18.toUByte(), utm.zone)
        assertEquals(UtmHemisphere.NORTH, utm.hemisphere)
        assertEquals(4_427_757.2188, utm.northing, 1e-3)

        val cell = h3Decode(h3Encode(wgs84(40.7128, -74.006), 9u))
        assertEquals(205.872_541, cell.maxErrorM, 1e-5)

        val d = geodesicDistanceM(wgs84(40.7128, -74.006), wgs84(51.5074, -0.1278))
        assertEquals(5_585_233.578_931, d, 1e-3)
    }

    @Test
    fun nmeaRmcCarriesItsInstant() {
        val rmc = fromNmeaSentence("\$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10")
        assertEquals(Instant.parse("1994-03-23T12:35:19Z"), rmc.timestamp)
    }

    @Test
    fun fixAccuracyIsTaggedAndValidated() {
        val gga = fromNmeaSentence("\$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47")
        assertEquals(AccuracyKind.ONE_SIGMA, gga.accuracy?.kind)
        assertEquals(46.9, gga.source!!.geoidSeparationM!!, 1e-12)
        assertEquals(AccuracyKind.BOUND, parseCoordinate("18TWK0000027757").accuracy?.kind)
        assertEquals(Accuracy(AccuracyKind.CEP50, 3.0, null), accuracyNew(AccuracyKind.CEP50, 3.0, null))
        val negative = throwsVariant<GeoException.InvalidValue> { accuracyNew(AccuracyKind.BOUND, -1.0, null) }
        assertEquals("horizontal", negative.field)
        val unsure = throwsVariant<GeoException.InvalidValue> { confidenceNew(Double.NaN) }
        assertEquals("confidence", unsure.field)
        val handBuilt = fixFromCoord(wgs84(40.7128, -74.006))
        handBuilt.accuracy = Accuracy(AccuracyKind.ONE_SIGMA, -1.0, null)
        throwsVariant<GeoException.InvalidValue> { formatFix(handBuilt, formatOptionsDefault()) }
    }

    @Test
    fun outOfRangeCarriesThePosition() {
        val e = throwsVariant<GeoException.OutOfRange> { coordinateNew(91.0, 0.0, Crs.WGS84) }
        assertEquals(91.0, e.lat)
        assertEquals(0.0, e.lon)
    }

    @Test
    fun crsMismatchCarriesBothSystems() {
        val e = throwsVariant<GeoException.CrsMismatch> { baiduMercatorFromCoordinate(wgs84(WGS.lat, WGS.lon)) }
        assertEquals(Crs.BD09, e.expected)
        assertEquals(Crs.WGS84, e.found)
    }

    @Test
    fun outsideDomainNamesTheOperation() {
        val pole = throwsVariant<GeoException.OutsideDomain> { gcj02ToBd09(Gcj02(90.0, 0.0)) }
        assertEquals("BD-09", pole.operation)
        val polar = throwsVariant<GeoException.OutsideDomain> { baiduMercatorFromBd09(Bd09(80.0, 0.0)) }
        assertEquals("Baidu Mercator", polar.operation)
    }

    @Test
    fun invalidValueNamesTheField() {
        val e = throwsVariant<GeoException.InvalidValue> { utmCentralMeridianDeg(0u) }
        assertEquals("zone", e.field)
    }

    @Test
    fun invalidCellIdNamesTheGridAndValue() {
        val e = throwsVariant<GeoException.InvalidCellId> { h3Decode(H3Cell(0uL)) }
        assertEquals("H3", e.grid)
        assertEquals(0uL, e.value)
    }

    @Test
    fun invalidGridRefCarriesTheReference() {
        val e = throwsVariant<GeoException.InvalidGridRef> { mgrsToCoordinate("not-an-mgrs") }
        assertEquals("not-an-mgrs", e.detail)
    }

    @Test
    fun parseRejectsAmbiguousTextAndNoFixNmea() {
        val ambiguous = throwsVariant<GeoException.Parse> { parseCoordinate("1234.5, 10") }
        assertEquals(ParseErrorKind.AMBIGUOUS, ambiguous.kind)
        val syntax = throwsVariant<GeoException.Parse> { parseCoordinate("ezs42") }
        assertEquals(ParseErrorKind.SYNTAX, syntax.kind)
        val void =
            throwsVariant<GeoException.Parse> {
                fromNmeaSentence("\$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07")
            }
        assertEquals(ParseErrorKind.NO_FIX, void.kind)
        assertTrue(void.detail.contains("void"), void.detail)
        val corrupt =
            throwsVariant<GeoException.Parse> {
                fromNmeaSentence("\$GPGLL,4916.45,N,12311.12,W,225444,A,*1E")
            }
        assertEquals(ParseErrorKind.CHECKSUM, corrupt.kind)
    }
}
