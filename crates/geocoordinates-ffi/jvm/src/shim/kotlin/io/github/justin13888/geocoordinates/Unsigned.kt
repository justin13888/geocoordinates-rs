// Java-facing shim over the UniFFI Kotlin bindings' unsigned integers.
//
// UniFFI maps `u8` / `u16` / `u32` / `u64` to Kotlin's `UByte` / `UShort` /
// `UInt` / `ULong`, which are inline classes: Kotlin mangles every JVM member
// that takes or exposes one (`getValue-s-VKNKU`, `s2Encode-EK-6454`), and javac
// cannot name the mangled form. Each holder object in this package re-exposes
// those members as `@JvmStatic` functions over Java's own integer types:
//
// - `u8` / `u16` cross as `int`, `u32` as `long`: the value itself, checked
//   against the unsigned type's range on the way in (`IllegalArgumentException`
//   naming the parameter), so no value is silently truncated.
// - `u64` crosses as `long` holding the same 64 bits, Java's own unsigned-long
//   convention: a value above 2^63 - 1 reads as negative, and
//   `Long.toUnsignedString` / `Long.parseUnsignedLong` /
//   `Long.compareUnsigned` treat it as unsigned. Every `long` is a valid `u64`.
// - An optional unsigned value crosses as a nullable `Integer` / `Long`, and a
//   list of `u64` as a `long[]`.
//
// A record whose unsigned field Java cannot set is rebuilt through its factory;
// the three records Java builds by field assignment (`ExifGps`, `Place`,
// `ReverseOptions`) also get a setter.
package io.github.justin13888.geocoordinates

internal fun toUByteArg(value: Int, name: String): UByte {
    require(value in 0..UByte.MAX_VALUE.toInt()) { "$name must be in [0, 255], got $value" }
    return value.toUByte()
}

internal fun toUShortArg(value: Int, name: String): UShort {
    require(value in 0..UShort.MAX_VALUE.toInt()) { "$name must be in [0, 65535], got $value" }
    return value.toUShort()
}

internal fun toUIntArg(value: Long, name: String): UInt {
    require(value in 0..UInt.MAX_VALUE.toLong()) { "$name must be in [0, 4294967295], got $value" }
    return value.toUInt()
}

internal fun toOptionalUByteArg(value: Int?, name: String): UByte? = value?.let { toUByteArg(it, name) }

internal fun LongArray.toULongList(): List<ULong> = map { it.toULong() }

internal fun List<ULong>.toLongArray(): LongArray = LongArray(size) { this[it].toLong() }
