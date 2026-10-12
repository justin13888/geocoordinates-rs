package io.github.justin13888.geocoordinates;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
// Top-level Kotlin functions compile to static methods on the file class.
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.baiduMercatorFromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.coordinateNew;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.gcj02ToWgs84Refined;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsFromCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsPrecision;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.mgrsToCoordinate;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.outOfChina;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84ToGcj02;

import org.junit.jupiter.api.Test;
import uniffi.geocoordinates_ffi.ApproxWgs84;
import uniffi.geocoordinates_ffi.Crs;
import uniffi.geocoordinates_ffi.Gcj02;
import uniffi.geocoordinates_ffi.GeoException;
import uniffi.geocoordinates_ffi.MgrsPrecision;
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
}
