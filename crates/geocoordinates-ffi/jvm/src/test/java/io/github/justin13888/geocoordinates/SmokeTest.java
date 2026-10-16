package io.github.justin13888.geocoordinates;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
// Top-level Kotlin functions compile to static methods on the file class.
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.accuracyNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.accuracyValidate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.baiduMercatorFromBd09;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.baiduMercatorFromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.confidenceNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.confidenceValidate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.coordinateNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.datumTransformCatalogToWgs84;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.ecefFromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.ellipsoidClarke1866;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.ellipsoidForCrs;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.fixFromCoord;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.formatFix;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.formatOptionsDefault;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.fromNmeaSentence;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.gcj02ToBd09;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.gcj02ToWgs84Refined;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsFromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsPrecision;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsToCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.outOfChina;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseCoordinateWith;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseOptionsDefault;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.rawSourceNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84FromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84ToCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84ToGcj02;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84Validate;

import java.time.Instant;
import org.junit.jupiter.api.Test;
import uniffi.geocoordinates_ffi.Accuracy;
import uniffi.geocoordinates_ffi.AccuracyKind;
import uniffi.geocoordinates_ffi.ApproxWgs84;
import uniffi.geocoordinates_ffi.AxisOrder;
import uniffi.geocoordinates_ffi.Bd09;
import uniffi.geocoordinates_ffi.Confidence;
import uniffi.geocoordinates_ffi.Coordinate;
import uniffi.geocoordinates_ffi.Crs;
import uniffi.geocoordinates_ffi.DatumTransform;
import uniffi.geocoordinates_ffi.DecimalSeparator;
import uniffi.geocoordinates_ffi.Ecef;
import uniffi.geocoordinates_ffi.Fix;
import uniffi.geocoordinates_ffi.Gcj02;
import uniffi.geocoordinates_ffi.GeoException;
import uniffi.geocoordinates_ffi.GridKind;
import uniffi.geocoordinates_ffi.MgrsPrecision;
import uniffi.geocoordinates_ffi.ParseOptions;
import uniffi.geocoordinates_ffi.RawSource;
import uniffi.geocoordinates_ffi.Wgs84;

/** Proves the Kotlin/JNA bindings are callable idiomatically from Java. */
class SmokeTest {
    // Every FFI function that returns `Result` surfaces as `@Throws(GeoException::class)`,
    // and Java treats `GeoException` as checked. Calling a newly fallible function without
    // declaring it fails `compileTestJava`, which CI reports only as the Gradle step exiting 1.
    @Test
    void chinaDatumRoundTripFromJava() throws GeoException {
        Wgs84 wgs = new Wgs84(39.915, 116.404);
        Gcj02 gcj = wgs84ToGcj02(wgs);

        // GCJ-02 applies a nonzero offset inside China.
        assertNotEquals(wgs.getLat(), gcj.getLat());
        assertNotEquals(wgs.getLon(), gcj.getLon());

        ApproxWgs84 back = gcj02ToWgs84Refined(gcj);
        assertEquals(39.915, back.getLat(), 1e-4);
        assertEquals(116.404, back.getLon(), 1e-4);
        assertTrue(back.getMaxErrorM() > 0.0);

        assertTrue(outOfChina(0.0, 0.0));
        assertFalse(outOfChina(39.915, 116.404));
    }

    // China records are validated as they cross in, and BD-09 / Baidu Mercator
    // report their domain edges as typed errors.
    @Test
    void chinaDomainEdgesFromJava() throws GeoException {
        assertThrows(GeoException.OutOfRange.class, () -> wgs84Validate(new Wgs84(91.0, 0.0)));
        Coordinate coord = wgs84ToCoordinate(new Wgs84(39.915, 116.404));
        assertEquals(Crs.WGS84, coord.getCrs());
        assertEquals(new Wgs84(39.915, 116.404), wgs84FromCoordinate(coord));

        GeoException.OutsideDomain pole =
                assertThrows(
                        GeoException.OutsideDomain.class, () -> gcj02ToBd09(new Gcj02(90.0, 0.0)));
        assertEquals("BD-09", pole.getOperation());
        GeoException.OutsideDomain polar =
                assertThrows(
                        GeoException.OutsideDomain.class,
                        () -> baiduMercatorFromBd09(new Bd09(80.0, 0.0)));
        assertEquals("Baidu Mercator", polar.getOperation());
    }

    // Each core error crosses as its own typed `GeoException` subclass, so Java
    // catches the variant and reads its fields instead of parsing a message.
    @Test
    void typedErrorsFromJava() {
        GeoException.CrsMismatch mismatch =
                assertThrows(
                        GeoException.CrsMismatch.class,
                        () ->
                                baiduMercatorFromCoordinate(
                                        coordinateNew(39.915, 116.404, Crs.WGS84)));
        assertEquals(Crs.BD09, mismatch.getExpected());
        assertEquals(Crs.WGS84, mismatch.getFound());

        GeoException.OutOfRange outOfRange =
                assertThrows(
                        GeoException.OutOfRange.class, () -> coordinateNew(91.0, 0.0, Crs.WGS84));
        assertEquals(91.0, outOfRange.getLat());
        assertEquals(0.0, outOfRange.getLon());

        GeoException.InvalidGridRef badGrid =
                assertThrows(
                        GeoException.InvalidGridRef.class, () -> mgrsToCoordinate("not-an-mgrs"));
        assertEquals("not-an-mgrs", badGrid.getDetail());
    }

    // MGRS precision crosses as the closed `MgrsPrecision` enum, so Java cannot
    // pass an off-grid square size.
    @Test
    void mgrsPrecisionEnumFromJava() throws GeoException {
        String code = mgrsFromCoordinate(coordinateNew(40.0, -75.0, Crs.WGS84), MgrsPrecision.M100);
        assertEquals("18TWK000277", code);
        assertEquals(MgrsPrecision.M100, mgrsPrecision(code));
    }

    // ECEF and datum transforms take the ellipsoid from the coordinate's CRS,
    // and a catalogued shift carries its published accuracy (a nullable Double).
    @Test
    void ellipsoidFollowsTheCrsFromJava() throws GeoException {
        Ecef nad27 = ecefFromCoordinate(coordinateNew(0.0, 0.0, Crs.NAD27));
        assertEquals(6_378_206.4, nad27.getX(), 1e-6);
        assertEquals(ellipsoidClarke1866(), ellipsoidForCrs(Crs.NAD27));
        assertNull(ellipsoidForCrs(Crs.GCJ02));
        GeoException.CrsMismatch noEllipsoid =
                assertThrows(
                        GeoException.CrsMismatch.class,
                        () -> ecefFromCoordinate(coordinateNew(39.9, 116.4, Crs.GCJ02)));
        assertEquals(Crs.WGS84, noEllipsoid.getExpected());

        DatumTransform tokyo = datumTransformCatalogToWgs84(Crs.TOKYO);
        double accuracy = tokyo.getAccuracyM();
        assertEquals(Math.sqrt(825.0), accuracy, 1e-12);
        assertNull(datumTransformCatalogToWgs84(Crs.PULKOVO42).getAccuracyM());
    }

    // MGRS is always detected; a geohash decodes only when `ParseOptions` expects
    // one (a nullable `GridKind`), and ambiguous free text is a typed `Parse` error.
    @Test
    void gridDetectionFromJava() throws GeoException {
        Fix mgrs = parseCoordinate("18TWK0000027757");
        assertNull(mgrs.getSource().getAxisOrder());
        assertThrows(GeoException.Parse.class, () -> parseCoordinate("ezs42"));
        assertThrows(GeoException.Parse.class, () -> parseCoordinate("1234.5, 10"));

        ParseOptions defaults = parseOptionsDefault();
        assertNull(defaults.getExpectGrid());
        ParseOptions geohash =
                new ParseOptions(AxisOrder.LAT_LON, DecimalSeparator.POINT, GridKind.GEOHASH);
        assertEquals(42.6, parseCoordinateWith("ezs42", geohash).getCoord().getLat(), 0.03);
    }

    // An NMEA RMC fix carries its UTC date and time as a `java.time.Instant`, and a
    // sentence reporting no fix (void status, GGA quality 0) is a typed `Parse` error.
    @Test
    void nmeaFixesFromJava() throws GeoException {
        Fix rmc =
                fromNmeaSentence(
                        "$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10");
        assertEquals(Instant.parse("1994-03-23T12:35:19Z"), rmc.getTimestamp());
        assertEquals(AxisOrder.LAT_LON, rmc.getSource().getAxisOrder());

        assertThrows(
                GeoException.Parse.class,
                () ->
                        fromNmeaSentence(
                                "$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07"));
        assertThrows(
                GeoException.Parse.class,
                () -> fromNmeaSentence("$GPGGA,123519,,,,,0,00,,,M,,M,,"));
    }

    // Fix metadata: every accuracy carries its kind, a GGA geoid separation is a
    // nullable Double, and Accuracy / Confidence are validated at construction and
    // again when a hand-built Fix crosses back in.
    @Test
    void fixMetadataFromJava() throws GeoException {
        Fix gga =
                fromNmeaSentence(
                        "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47");
        assertEquals(AccuracyKind.ONE_SIGMA, gga.getAccuracy().getKind());
        double separation = gga.getSource().getGeoidSeparationM();
        assertEquals(46.9, separation, 1e-12);
        assertEquals(
                AccuracyKind.BOUND, parseCoordinate("18TWK0000027757").getAccuracy().getKind());

        Accuracy cep = accuracyNew(AccuracyKind.CEP50, 3.0, null);
        assertEquals(new Accuracy(AccuracyKind.CEP50, 3.0, null), cep);
        accuracyValidate(cep);
        GeoException.InvalidValue negative =
                assertThrows(
                        GeoException.InvalidValue.class,
                        () -> accuracyNew(AccuracyKind.BOUND, -1.0, null));
        assertEquals("horizontal", negative.getField());
        GeoException.InvalidValue unsure =
                assertThrows(GeoException.InvalidValue.class, () -> confidenceNew(1.5));
        assertEquals("confidence", unsure.getField());
        assertThrows(
                GeoException.InvalidValue.class,
                () -> confidenceValidate(new Confidence(Double.NaN)));
        RawSource source = rawSourceNew("51.5, -0.12", confidenceNew(0.9));
        assertNull(source.getGeoidSeparationM());

        Fix handBuilt = fixFromCoord(coordinateNew(40.7128, -74.006, Crs.WGS84));
        handBuilt.setAccuracy(new Accuracy(AccuracyKind.ONE_SIGMA, -1.0, null));
        assertThrows(
                GeoException.InvalidValue.class,
                () -> formatFix(handBuilt, formatOptionsDefault()));
    }
}
