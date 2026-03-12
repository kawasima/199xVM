/*
 * Copyright (c) 1996, 2024, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */

package java.lang;

public final class Double extends Number implements Comparable<Double> {
    public static final double POSITIVE_INFINITY = 1.0d / 0.0d;
    public static final double NEGATIVE_INFINITY = -1.0d / 0.0d;
    public static final double NaN = 0.0d / 0.0d;
    public static final int MAX_EXPONENT = 1023;
    public static final int MIN_EXPONENT = -1022;
    public static final int SIZE = 64;
    public static final double MAX_VALUE = 0x1.fffffffffffffP+1023; // 1.7976931348623157E308
    public static final double MIN_VALUE = 0x0.0000000000001P-1022; // 4.9E-324
    public static final int BYTES = 8;
    public static final int PRECISION = 53;
    @SuppressWarnings("unchecked")
    public static final Class<Double> TYPE = (Class<Double>) primitiveType("double");
    private static Class<?> primitiveType(String name) { try { return Class.forName(name); } catch (ClassNotFoundException e) { return null; } }
    private final double value;

    public Double(double value) { this.value = value; }

    public static Double valueOf(double d) { return new Double(d); }

    @Override public int intValue() { return (int) value; }
    @Override public long longValue() { return (long) value; }
    @Override public float floatValue() { return (float) value; }
    @Override public double doubleValue() { return value; }

    @Override public String toString() { return toString(value); }

    public static native String toString(double d);

    @Override public int hashCode() { return (int) doubleToLongBits(value); }

    @Override
    public boolean equals(Object obj) {
        return (obj instanceof Double other) && doubleToLongBits(value) == doubleToLongBits(other.value);
    }

    @Override
    public int compareTo(Double anotherDouble) {
        return compare(this.value, anotherDouble.value);
    }

    public static int compare(double d1, double d2) {
        if (d1 < d2) return -1;
        if (d1 > d2) return 1;
        return 0;
    }

    public static boolean isInfinite(double v) {
        return v == POSITIVE_INFINITY || v == NEGATIVE_INFINITY;
    }

    public static boolean isNaN(double v) {
        return v != v;
    }

    public static boolean isFinite(double v) {
        return !isInfinite(v) && !isNaN(v);
    }

    public static int hashCode(double value) {
        long bits = doubleToLongBits(value);
        return (int)(bits ^ (bits >>> 32));
    }

    public static native long doubleToLongBits(double value);
    public static long doubleToRawLongBits(double value) { return doubleToLongBits(value); }
    public static native double longBitsToDouble(long bits);
}
