package io.github.justin13888.geocoordinates;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
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
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.clusterIndexOptionsDefault;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.coarsen;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.confidenceNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.confidenceValidate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.coordinateNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.datumTransformCatalogToWgs84;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.datumTransformValidate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.dmsToDegrees;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.ecefFromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.ecefValidate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.ellipsoidClarke1866;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.ellipsoidForCrs;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.ellipsoidValidate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.fixFromCoord;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.formatCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.formatFix;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.formatOptionsDefault;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.gcj02IsOutsideChina;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.gcj02ToBd09;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.gcj02ToWgs84Refined;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.geotagOptionsDefault;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.h3Decode;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.heightMeters;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.lengthUnitMetersPerUnit;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsFromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsPrecision;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsToCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseCoordinateWith;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseNmeaSentence;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseOptionsDefault;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseExif;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.parseWkt;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.placeNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.rawSourceNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.reverseOptionsDefault;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.s2Decode;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.s2Validate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.tileValidate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.upsToCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.utmFromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84FromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84IsOutsideChina;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84ToCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84ToGcj02;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84Validate;

import java.time.Instant;
import java.util.List;
import org.junit.jupiter.api.Test;
import uniffi.geocoordinates_ffi.Accuracy;
import uniffi.geocoordinates_ffi.AccuracyKind;
import uniffi.geocoordinates_ffi.ApproxCoordinate;
import uniffi.geocoordinates_ffi.ApproxWgs84;
import uniffi.geocoordinates_ffi.AxisOrder;
import uniffi.geocoordinates_ffi.Bd09;
import uniffi.geocoordinates_ffi.Cluster;
import uniffi.geocoordinates_ffi.ClusterIndex;
import uniffi.geocoordinates_ffi.Confidence;
import uniffi.geocoordinates_ffi.Coordinate;
import uniffi.geocoordinates_ffi.Crs;
import uniffi.geocoordinates_ffi.DatumTransform;
import uniffi.geocoordinates_ffi.DecimalSeparator;
import uniffi.geocoordinates_ffi.DbscanResult;
import uniffi.geocoordinates_ffi.Ecef;
import uniffi.geocoordinates_ffi.ExifDms;
import uniffi.geocoordinates_ffi.ExifGps;
import uniffi.geocoordinates_ffi.ExifRational;
import uniffi.geocoordinates_ffi.Fix;
import uniffi.geocoordinates_ffi.FormatOptions;
import uniffi.geocoordinates_ffi.Gazetteer;
import uniffi.geocoordinates_ffi.Gcj02;
import uniffi.geocoordinates_ffi.GeoBounds;
import uniffi.geocoordinates_ffi.GeoException;
import uniffi.geocoordinates_ffi.GridKind;
import uniffi.geocoordinates_ffi.H3Cell;
import uniffi.geocoordinates_ffi.Height;
import uniffi.geocoordinates_ffi.Hemisphere;
import uniffi.geocoordinates_ffi.HemisphereStyle;
import uniffi.geocoordinates_ffi.MapCluster;
import uniffi.geocoordinates_ffi.LengthUnit;
import uniffi.geocoordinates_ffi.MgrsPrecision;
import uniffi.geocoordinates_ffi.ParseErrorKind;
import uniffi.geocoordinates_ffi.ParseOptions;
import uniffi.geocoordinates_ffi.Place;
import uniffi.geocoordinates_ffi.PointLabel;
import uniffi.geocoordinates_ffi.Precision;
import uniffi.geocoordinates_ffi.RawSource;
import uniffi.geocoordinates_ffi.ReverseOptions;
import uniffi.geocoordinates_ffi.S2CellId;
import uniffi.geocoordinates_ffi.SnapGrid;
import uniffi.geocoordinates_ffi.SpatialIndex;
import uniffi.geocoordinates_ffi.SpatialNeighbor;
import uniffi.geocoordinates_ffi.SymbolStyle;
import uniffi.geocoordinates_ffi.Tile;
import uniffi.geocoordinates_ffi.Track;
import uniffi.geocoordinates_ffi.Ups;
import uniffi.geocoordinates_ffi.Utm;
import uniffi.geocoordinates_ffi.UtmHemisphere;
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

        assertTrue(wgs84IsOutsideChina(new Wgs84(0.0, 0.0)));
        assertFalse(wgs84IsOutsideChina(wgs));
        assertFalse(gcj02IsOutsideChina(gcj));
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
        assertEquals("MGRS", badGrid.getGrid());
        assertEquals("\"not-an-mgrs\" is not a well-formed reference", badGrid.getDetail());

        // On the grid, but its corner decodes near 64°N, outside the 84°N cap.
        GeoException.InvalidValue offCap =
                assertThrows(
                        GeoException.InvalidValue.class,
                        () -> upsToCoordinate(new Ups(UtmHemisphere.NORTH, 0.0, 0.0)));
        assertEquals("northing", offCap.getField());
    }

    // MGRS precision crosses as the closed `MgrsPrecision` enum, so Java cannot
    // pass an off-grid square size.
    @Test
    void mgrsPrecisionEnumFromJava() throws GeoException {
        String code = mgrsFromCoordinate(coordinateNew(40.0, -75.0, Crs.WGS84), MgrsPrecision.M100);
        assertEquals("18TWK000277", code);
        assertEquals(MgrsPrecision.M100, mgrsPrecision(code));
    }

    // Either height surface reads back in meters without a type switch, and a
    // length unit states its size in meters.
    @Test
    void heightAndUnitMetersFromJava() {
        assertEquals(46.9, heightMeters(new Height.Ellipsoidal(46.9)), 0.0);
        assertEquals(-28.0, heightMeters(new Height.Orthometric(-28.0)), 0.0);
        assertEquals(1852.0, lengthUnitMetersPerUnit(LengthUnit.NAUTICAL_MILE), 0.0);
    }

    // ECEF and datum transforms take the ellipsoid from the coordinate's CRS,
    // and a catalogued shift carries its published accuracy (a nullable Double).
    @Test
    void ellipsoidFollowsTheCrsFromJava() throws GeoException {
        Ecef nad27 = ecefFromCoordinate(coordinateNew(0.0, 0.0, Crs.NAD27));
        assertEquals(6_378_206.4, nad27.getXM(), 1e-6);
        assertEquals(ellipsoidClarke1866(), ellipsoidForCrs(Crs.NAD27));
        assertNull(ellipsoidForCrs(Crs.GCJ02));
        GeoException.CrsMismatch noEllipsoid =
                assertThrows(
                        GeoException.CrsMismatch.class,
                        () -> ecefFromCoordinate(coordinateNew(39.9, 116.4, Crs.GCJ02)));
        assertEquals(Crs.WGS84, noEllipsoid.getExpected());
        // A hand-built ECEF record is validated, naming the non-finite component.
        ecefValidate(nad27);
        GeoException.InvalidValue notFinite =
                assertThrows(
                        GeoException.InvalidValue.class,
                        () -> ecefValidate(new Ecef(Double.NaN, 0.0, 0.0)));
        assertEquals("x_m", notFinite.getField());

        DatumTransform tokyo = datumTransformCatalogToWgs84(Crs.TOKYO);
        double accuracy = tokyo.getAccuracyM();
        assertEquals(Math.sqrt(825.0), accuracy, 1e-12);
        assertNull(datumTransformCatalogToWgs84(Crs.PULKOVO42).getAccuracyM());
        // A record this library returned validates; the validators return void.
        ellipsoidValidate(ellipsoidClarke1866());
        datumTransformValidate(tokyo);
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
                parseNmeaSentence(
                        "$GPRMC,123519,A,3340.000,S,07030.000,W,000.0,000.0,230394,,*10");
        assertEquals(Instant.parse("1994-03-23T12:35:19Z"), rmc.getTimestamp());
        assertEquals(AxisOrder.LAT_LON, rmc.getSource().getAxisOrder());

        assertThrows(
                GeoException.Parse.class,
                () ->
                        parseNmeaSentence(
                                "$GPRMC,123519,V,3340.000,S,07030.000,W,000.0,000.0,230394,,*07"));
        assertThrows(
                GeoException.Parse.class,
                () -> parseNmeaSentence("$GPGGA,123519,,,,,0,00,,,M,,M,,"));
        // Each rejection names its kind: a sentence type with no position, and a
        // document nested past its format's limit.
        GeoException.Parse unsupported =
                assertThrows(GeoException.Parse.class, () -> parseNmeaSentence("$GPVTG,054.7,T"));
        assertEquals(ParseErrorKind.UNSUPPORTED, unsupported.getKind());
        GeoException.Parse deep =
                assertThrows(
                        GeoException.Parse.class,
                        () -> parseWkt("GEOMETRYCOLLECTION(".repeat(200), AxisOrder.LON_LAT));
        assertEquals(ParseErrorKind.TOO_DEEP, deep.getKind());
    }

    // A `java.time.Instant` before 1970 with a fractional second crosses into a
    // Track and back unchanged. The fixes are about 56 m apart, inside the
    // default 50 m/s speed limit; every instant is at least a second before the
    // epoch, since one inside the last second crosses with its sign flipped (#342).
    @Test
    void preEpochAndSubsecondTimestampsRoundTripThroughATrack() throws GeoException {
        Instant before = Instant.parse("1969-12-31T23:59:56.250Z");
        Instant after = Instant.parse("1969-12-31T23:59:58.250Z");
        Track track =
                new Track(
                        List.of(
                                new Fix(coordinateNew(0.0, 0.0005, Crs.WGS84), null, after, null),
                                new Fix(coordinateNew(0.0, 0.0, Crs.WGS84), null, before, null)));
        List<Fix> fixes = track.fixes();
        assertEquals(before, fixes.get(0).getTimestamp());
        assertEquals(after, fixes.get(1).getTimestamp());
        Instant midway = Instant.parse("1969-12-31T23:59:57.250Z");
        Fix between = track.locate(midway, geotagOptionsDefault());
        assertEquals(midway, between.getTimestamp());
        assertEquals(0.00025, between.getCoord().getLon(), 1e-12);
    }

    // Fix metadata: every accuracy carries its kind, a GGA geoid separation is a
    // nullable Double, and Accuracy / Confidence are validated at construction and
    // again when a hand-built Fix crosses back in.
    @Test
    void fixMetadataFromJava() throws GeoException {
        Fix gga =
                parseNmeaSentence(
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

    // Kotlin mangles every member over an unsigned type, so Java reaches them
    // through the shim's holders: a `u64` crosses as the same 64 bits in a `long`
    // (an S2 id above 2^63 - 1 reads as negative), and a smaller unsigned value
    // as its own `int` or `long`, range-checked on the way in.
    @Test
    void s2CellIdsAboveTwoToThe63CrossFromJava() throws GeoException {
        Coordinate nyc = coordinateNew(40.7128, -74.006, Crs.WGS84);
        S2CellId cell = S2Cells.s2Encode(nyc, 20);
        long id = S2Cells.toLong(cell);
        assertTrue(id < 0);
        assertEquals("9926595630970437632", Long.toUnsignedString(id));
        assertEquals(20, S2Cells.s2Level(cell));
        s2Validate(cell);
        assertEquals(cell, S2Cells.fromLong(id));
        ApproxCoordinate back =
                s2Decode(S2Cells.fromLong(Long.parseUnsignedLong("9926595630970437632")));
        assertEquals(40.712_762_829, back.getCoord().getLat(), 1e-8);
        assertEquals(-74.006_004_053, back.getCoord().getLon(), 1e-8);

        // The id an error carries crosses back just as wide.
        GeoException.InvalidCellId bad =
                assertThrows(
                        GeoException.InvalidCellId.class, () -> s2Decode(S2Cells.fromLong(-1L)));
        assertEquals("S2", bad.getGrid());
        assertEquals(
                "18446744073709551615", Long.toUnsignedString(GeoErrors.invalidCellIdValue(bad)));
        assertEquals(-1L, GeoErrors.invalidCellIdValue(GeoErrors.invalidCellId("S2", -1L)));

        IllegalArgumentException wide =
                assertThrows(IllegalArgumentException.class, () -> S2Cells.s2Encode(nyc, 256));
        assertEquals("level must be in [0, 255], got 256", wide.getMessage());

        H3Cell h3 = H3Cells.h3Encode(nyc, 9);
        assertEquals(9, H3Cells.h3Resolution(h3));
        assertEquals(h3, H3Cells.fromLong(H3Cells.toLong(h3)));
        assertEquals(40.7128, h3Decode(h3).getCoord().getLat(), 1e-2);
    }

    @Test
    void gridsAnglesAndFormatsTakeIntsFromJava() throws GeoException {
        Coordinate nyc = coordinateNew(40.7128, -74.006, Crs.WGS84);
        assertEquals("dr5re", Grids.geohashEncode(nyc, 5));
        assertEquals("FN20", Grids.maidenheadEncode(nyc, 2));
        assertEquals(11, Grids.plusCodeEncode(nyc, 10).length());
        assertEquals(100L, Grids.mgrsPrecisionMeters(MgrsPrecision.M100));
        Utm utm = utmFromCoordinate(nyc);
        assertEquals(18, Grids.utmZone(utm));
        assertEquals(18, Grids.utmZoneFor(nyc));
        assertEquals(-75.0, Grids.utmCentralMeridianDeg(18), 0.0);
        assertEquals(
                utm, Grids.utm(18, utm.getHemisphere(), utm.getEastingM(), utm.getNorthingM()));

        assertEquals(
                40.7128, dmsToDegrees(Angles.dms(40, 42, 46.08, Hemisphere.NORTH)), 1e-9);
        assertEquals(42, Angles.dmsMinutes(Angles.dms(40, 42, 46.08, Hemisphere.NORTH)));
        assertEquals(40, Angles.ddmDegrees(Angles.ddm(40, 42.768, Hemisphere.NORTH)));
        assertThrows(
                IllegalArgumentException.class, () -> Angles.dms(-1, 0, 0.0, Hemisphere.NORTH));

        FormatOptions geohash = new FormatOptions(Formats.geohash(5), DecimalSeparator.POINT);
        assertEquals("dr5re", formatCoordinate(nyc, geohash));
        assertEquals(5, Formats.geohashLength(Formats.geohash(5)));
        HemisphereStyle signed = HemisphereStyle.SIGNED;
        assertNull(Formats.decimalDegreesDecimals(Formats.decimalDegrees(null, signed)));
        assertEquals(3, Formats.decimalDegreesDecimals(Formats.decimalDegrees(3, signed)));
        SymbolStyle unicode = SymbolStyle.UNICODE;
        assertEquals(2, Formats.dmsSecondDecimals(Formats.dms(2, unicode, signed)));
        assertEquals(4, Formats.ddmMinuteDecimals(Formats.ddm(4, unicode, signed)));
        assertEquals(10, Formats.plusCodeLength(Formats.plusCode(10)));
        assertEquals(3, Formats.maidenheadPairs(Formats.maidenhead(3)));
        assertEquals(40, Angles.dmsDegrees(Angles.dms(40, 42, 46.08, Hemisphere.NORTH)));
    }

    @Test
    void tilesAndPrecisionTakeIntsFromJava() throws GeoException {
        Coordinate nyc = coordinateNew(40.7128, -74.006, Crs.WGS84);
        Tile tile = Tiles.tileFromCoordinate(nyc, 10);
        assertEquals(10, Tiles.tileZ(tile));
        assertEquals(301L, Tiles.tileX(tile));
        assertEquals(385L, Tiles.tileY(tile));
        assertEquals(tile, Tiles.tile(10, 301L, 385L));
        tileValidate(tile);
        assertTrue(Tiles.tileMaxZoom() >= 10);
        GeoBounds box = new GeoBounds(-10.0, -10.0, 10.0, 10.0, Crs.WGS84);
        assertEquals(1, Tiles.tilesCovering(box, 0, 4L).size());
        assertThrows(IllegalArgumentException.class, () -> Tiles.tile(0, 1L << 32, 0L));

        Precision precision = Privacy.precision(SnapGrid.GEOHASH, 5);
        assertEquals(5, Privacy.precisionLevel(precision));
        assertNotEquals(nyc, coarsen(nyc, precision).getCoord());
    }

    @Test
    void clusterIdsAndCountsCrossAsLongsFromJava() throws GeoException {
        List<Coordinate> points =
                List.of(
                        coordinateNew(40.0, -74.0, Crs.WGS84),
                        coordinateNew(40.0001, -74.0, Crs.WGS84),
                        coordinateNew(10.0, 10.0, Crs.WGS84));
        DbscanResult result = Clusters.dbscan(points, 100.0, 2L);
        Cluster cluster = result.getClusters().get(0);
        assertArrayEquals(new long[] {0L, 1L}, Clusters.clusterMembers(cluster));
        assertEquals(2L, Clusters.clusterCount(cluster));
        PointLabel.Cluster label = (PointLabel.Cluster) result.getLabels().get(0);
        assertEquals(Clusters.clusterId(cluster), Clusters.pointLabelClusterId(label));
        assertEquals(label, Clusters.pointLabelCluster(Clusters.clusterId(cluster)));
        assertTrue(result.getLabels().get(2) instanceof PointLabel.Noise);
        assertEquals(
                cluster,
                Clusters.cluster(
                        Clusters.clusterId(cluster),
                        cluster.getCentroid(),
                        cluster.getCentroidMaxErrorM(),
                        cluster.getBounds(),
                        2L,
                        new long[] {0L, 1L}));
        assertEquals(1, Clusters.bucketByGeohash(points.subList(0, 2), 5).size());
        assertEquals(1, Clusters.bucketByH3(points.subList(0, 2), 7).size());
        assertArrayEquals(
                new long[] {0L, 1L},
                Clusters.tripVisits(
                        Clusters.trip(
                                new long[] {0L, 1L},
                                Instant.EPOCH,
                                Instant.EPOCH.plusSeconds(60),
                                cluster.getBounds())));

        ClusterIndex index = new ClusterIndex(points, clusterIndexOptionsDefault());
        assertEquals(3L, Clusters.clusterIndexLen(index));
        long added = Clusters.clusterIndexInsert(index, coordinateNew(-30.0, 20.0, Crs.WGS84));
        assertEquals(-30.0, Clusters.clusterIndexCoordinate(index, added).getLat(), 0.0);
        Clusters.clusterIndexRemove(index, added);
        assertNull(Clusters.clusterIndexCoordinate(index, added));
        GeoBounds globe = new GeoBounds(-90.0, -180.0, 90.0, 180.0, Crs.WGS84);
        List<MapCluster> world =
                Clusters.clusterIndexClusters(index, globe, Clusters.clusterMaxZoom());
        assertEquals(3L, world.stream().mapToLong(Clusters::mapClusterCount).sum());
        MapCluster first = world.get(0);
        assertEquals(
                first,
                Clusters.mapCluster(
                        Clusters.mapClusterId(first),
                        Clusters.mapClusterZoom(first),
                        first.getCentroid(),
                        first.getCentroidMaxErrorM(),
                        first.getBounds(),
                        Clusters.mapClusterCount(first)));
        MapCluster pair =
                Clusters.clusterIndexClusters(index, globe, 0).stream().filter(c -> Clusters.mapClusterCount(c) == 2L).findFirst().get();
        long[] leaves = Clusters.clusterIndexLeaves(index, Clusters.mapClusterId(pair), 10L, 0L);
        java.util.Arrays.sort(leaves);
        assertArrayEquals(new long[] {0L, 1L}, leaves);
        assertFalse(Clusters.clusterIndexChildren(index, Clusters.mapClusterId(pair)).isEmpty());

        SpatialIndex spatial = new SpatialIndex(points);
        assertEquals(3L, SpatialIndexes.spatialIndexLen(spatial));
        SpatialNeighbor nearest =
                SpatialIndexes.spatialIndexNearest(
                                spatial, coordinateNew(10.1, 10.0, Crs.WGS84), 1L)
                        .get(0);
        assertEquals(2L, SpatialIndexes.spatialNeighborIndex(nearest));
        assertEquals(
                nearest,
                SpatialIndexes.spatialNeighbor(
                        nearest.getCoordinate(), 2L, nearest.getDistanceM()));

        Instant second = Instant.EPOCH.plusSeconds(1);
        Track track = new Track(List.of(new Fix(points.get(0), null, second, null)));
        assertEquals(1L, Tracks.trackLen(track));
        assertEquals(0L, Tracks.trackDroppedUntimed(track));
    }

    @Test
    void placesAndExifTagsSetUnsignedFieldsFromJava() throws GeoException {
        Coordinate nyc = coordinateNew(40.7128, -74.006, Crs.WGS84);
        Place place = placeNew("Somewhere", nyc, "US", nyc, 0.0);
        assertNull(Places.placePopulation(place));
        long most = Long.parseUnsignedLong("18446744073709551615");
        Places.setPlacePopulation(place, most);
        assertEquals(most, Places.placePopulation(place));

        // The fixture the Kotlin suite reads, relative to the Gradle project.
        Gazetteer gazetteer =
                Gazetteer.Companion.open("../../../src/reverse/data/fixture.gaz");
        assertTrue(Gazetteers.gazetteerLen(gazetteer) > 0L);

        ReverseOptions options = reverseOptionsDefault();
        Places.setReverseOptionsMinPopulation(options, 1_000L);
        assertEquals(1_000L, Places.reverseOptionsMinPopulation(options));
        assertEquals(
                options,
                Places.reverseOptions(
                        options.getLanguages(),
                        options.getMaxDistanceM(),
                        1_000L,
                        options.getMinFeatureClass()));

        ExifRational.Unsigned ten = Exif.unsignedRational(10L, 1L);
        assertEquals(10L, Exif.unsignedRationalNumerator(ten));
        assertEquals(1L, Exif.unsignedRationalDenominator(ten));
        assertEquals(
                4_294_967_295L,
                Exif.unsignedRationalNumerator(Exif.unsignedRational(4_294_967_295L, 1L)));
        ExifGps gps = Exif.exifGps();
        gps.setLatitude(
                new ExifDms(
                        Exif.unsignedRational(39L, 1L),
                        Exif.unsignedRational(54L, 1L),
                        Exif.unsignedRational(0L, 1L)));
        gps.setLatitudeRef("N");
        gps.setLongitude(
                new ExifDms(
                        Exif.unsignedRational(116L, 1L),
                        Exif.unsignedRational(24L, 1L),
                        Exif.unsignedRational(0L, 1L)));
        gps.setLongitudeRef("E");
        gps.setAltitude(ten);
        Exif.setExifGpsAltitudeRef(gps, 1);
        assertEquals(1, Exif.exifGpsAltitudeRef(gps));
        assertEquals(new Height.Orthometric(-10.0), parseExif(gps).getFix().getCoord().getHeight());
    }
}
