package io.github.justin13888.geocoordinates;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
// Top-level Kotlin functions compile to static methods on the file class.
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.gcj02ToWgs84Refined;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.outOfChina;
import static uniffi.geocoordinates_ffi.Geocoordinates_ffiKt.wgs84ToGcj02;

import org.junit.jupiter.api.Test;
import uniffi.geocoordinates_ffi.ApproxWgs84;
import uniffi.geocoordinates_ffi.Gcj02;
import uniffi.geocoordinates_ffi.GeoException;
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
}
