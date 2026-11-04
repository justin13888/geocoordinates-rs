package io.github.justin13888.geocoordinates

import java.io.File
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows
import uniffi.geocoordinates_ffi.Coordinate
import uniffi.geocoordinates_ffi.Crs
import uniffi.geocoordinates_ffi.FeatureClass
import uniffi.geocoordinates_ffi.Gazetteer
import uniffi.geocoordinates_ffi.GeoException
import uniffi.geocoordinates_ffi.ParseErrorKind
import uniffi.geocoordinates_ffi.Place
import uniffi.geocoordinates_ffi.ReverseGeocoder
import uniffi.geocoordinates_ffi.ReverseOptions
import uniffi.geocoordinates_ffi.coordinateNew
import uniffi.geocoordinates_ffi.gazetteerAsReverseGeocoder
import uniffi.geocoordinates_ffi.placeNew
import uniffi.geocoordinates_ffi.reverseGeocode
import uniffi.geocoordinates_ffi.reverseOptionsDefault

/** The core crate's doctest gazetteer; Gradle runs the tests from `jvm/`. */
private val fixture = File("../../../src/reverse/data/fixture.gaz")

private fun at(lat: Double, lon: Double) = coordinateNew(lat, lon, Crs.WGS84)

/** A foreign geocoder, as a platform-provider adapter would be written. */
private class Everywhere(private val countryCode: String) : ReverseGeocoder {
    override fun reverse(coordinate: Coordinate, options: ReverseOptions): Place? =
        if (coordinate.lat < -80.0) {
            null
        } else {
            placeNew("Everywhere", at(0.0, 0.0), countryCode, coordinate, 0.0)
        }
}

class ReverseBindingsTest {
    @Test
    fun gazetteerLoadsAndResolves() {
        val gazetteer = Gazetteer.fromBytes(fixture.readBytes())
        assertEquals(4uL, gazetteer.len())
        assertEquals(listOf("fr", "ja"), gazetteer.languages())
        assertEquals("2026-11-03", Gazetteer.open(fixture.path).sourceDate())
        val options = reverseOptionsDefault().copy(languages = listOf("ja"))
        val paris = gazetteer.reverse(at(48.86, 2.35), options)!!
        assertEquals("Paris", paris.name)
        assertEquals("パリ", paris.localizedNames["ja"])
        assertEquals(FeatureClass.CAPITAL, paris.featureClass)
        assertNull(gazetteer.reverse(at(0.0, -30.0), reverseOptionsDefault()))
        val truncated = assertThrows<GeoException.Parse> { Gazetteer.fromBytes("GCGZ".toByteArray()) }
        assertEquals(ParseErrorKind.SYNTAX, truncated.kind)
    }

    @Test
    fun aKotlinGeocoderCrossesIntoRust() {
        val found = reverseGeocode(Everywhere("XX"), at(0.0, 1.0), reverseOptionsDefault())!!
        assertEquals("Everywhere", found.name)
        assertTrue(found.distanceM > 100_000.0)
        assertNull(reverseGeocode(Everywhere("XX"), at(-85.0, 0.0), reverseOptionsDefault()))
        val lying = assertThrows<GeoException.InvalidValue> {
            reverseGeocode(Everywhere("nowhere"), at(0.0, 1.0), reverseOptionsDefault())
        }
        assertEquals("country_code", lying.field)
        val gazetteer = gazetteerAsReverseGeocoder(Gazetteer.fromBytes(fixture.readBytes()))
        assertEquals("Lyon", reverseGeocode(gazetteer, at(45.75, 4.85), reverseOptionsDefault())?.name)
    }
}
