/*
 * Copyright (c) 1999, 2025, Oracle and/or its affiliates. All rights reserved.
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

/**
 * The class {@code StrictMath} contains methods for performing basic
 * numeric operations such as the elementary exponential, logarithm,
 * square root, and trigonometric functions.
 *
 * <p>JDK 25 source — transcendental functions replaced with native
 * stubs (backed by Rust {@code f64} intrinsics) in place of the
 * {@code FdLibm} / {@code jdk.internal.math} dependency.
 *
 * @since 1.3
 */
public final class StrictMath {

    /**
     * Don't let anyone instantiate this class.
     */
    private StrictMath() {}

    /**
     * The {@code double} value that is closer than any other to
     * <i>e</i>, the base of the natural logarithms.
     */
    public static final double E = 2.718281828459045;

    /**
     * The {@code double} value that is closer than any other to
     * <i>pi</i> (&pi;), the ratio of the circumference of a circle to
     * its diameter.
     */
    public static final double PI = 3.141592653589793;

    /**
     * The {@code double} value that is closer than any other to
     * <i>tau</i> (&tau;), the ratio of the circumference of a circle
     * to its radius.
     *
     * @since 19
     */
    public static final double TAU = 2.0 * PI;

    // ── Transcendental / FdLibm replacements (native stubs) ─────────

    public static native double sin(double a);
    public static native double cos(double a);
    public static native double tan(double a);
    public static native double asin(double a);
    public static native double acos(double a);
    public static native double atan(double a);

    public static native double exp(double a);
    public static native double log(double a);
    public static native double log10(double a);
    public static native double sqrt(double a);
    public static native double cbrt(double a);

    public static native double IEEEremainder(double f1, double f2);
    public static native double ceil(double a);
    public static native double floor(double a);
    public static native double rint(double a);
    public static native double atan2(double y, double x);
    public static native double pow(double a, double b);

    public static native double sinh(double x);
    public static native double cosh(double x);
    public static native double tanh(double x);
    public static native double hypot(double x, double y);
    public static native double expm1(double x);
    public static native double log1p(double x);

    // ── Delegations to java.lang.Math (identical to JDK 25) ─────────

    public static double toRadians(double angdeg) {
        return Math.toRadians(angdeg);
    }

    public static double toDegrees(double angrad) {
        return Math.toDegrees(angrad);
    }

    public static int round(float a) {
        return Math.round(a);
    }

    public static long round(double a) {
        return Math.round(a);
    }

    public static double random() {
        return Math.random();
    }

    // ── Exact arithmetic ────────────────────────────────────────────

    public static int addExact(int x, int y) {
        return Math.addExact(x, y);
    }

    public static long addExact(long x, long y) {
        return Math.addExact(x, y);
    }

    public static int subtractExact(int x, int y) {
        return Math.subtractExact(x, y);
    }

    public static long subtractExact(long x, long y) {
        return Math.subtractExact(x, y);
    }

    public static int multiplyExact(int x, int y) {
        return Math.multiplyExact(x, y);
    }

    public static long multiplyExact(long x, long y) {
        return Math.multiplyExact(x, y);
    }

    public static int incrementExact(int a) {
        return Math.incrementExact(a);
    }

    public static long incrementExact(long a) {
        return Math.incrementExact(a);
    }

    public static int decrementExact(int a) {
        return Math.decrementExact(a);
    }

    public static long decrementExact(long a) {
        return Math.decrementExact(a);
    }

    public static int negateExact(int a) {
        return Math.negateExact(a);
    }

    public static long negateExact(long a) {
        return Math.negateExact(a);
    }

    public static int toIntExact(long value) {
        return Math.toIntExact(value);
    }

    // ── Floor / Ceil division ───────────────────────────────────────

    public static int floorDiv(int x, int y) {
        return Math.floorDiv(x, y);
    }

    public static long floorDiv(long x, long y) {
        return Math.floorDiv(x, y);
    }

    public static int floorMod(int x, int y) {
        return Math.floorMod(x, y);
    }

    public static long floorMod(long x, long y) {
        return Math.floorMod(x, y);
    }

    // ── Absolute value ──────────────────────────────────────────────

    public static int abs(int a) {
        return Math.abs(a);
    }

    public static long abs(long a) {
        return Math.abs(a);
    }

    public static float abs(float a) {
        return Math.abs(a);
    }

    public static double abs(double a) {
        return Math.abs(a);
    }

    // ── Max / Min ───────────────────────────────────────────────────

    public static int max(int a, int b) {
        return Math.max(a, b);
    }

    public static long max(long a, long b) {
        return Math.max(a, b);
    }

    public static float max(float a, float b) {
        return Math.max(a, b);
    }

    public static double max(double a, double b) {
        return Math.max(a, b);
    }

    public static int min(int a, int b) {
        return Math.min(a, b);
    }

    public static long min(long a, long b) {
        return Math.min(a, b);
    }

    public static float min(float a, float b) {
        return Math.min(a, b);
    }

    public static double min(double a, double b) {
        return Math.min(a, b);
    }

    // ── Sign / Copy ─────────────────────────────────────────────────

    public static double signum(double d) {
        return Math.signum(d);
    }

    public static float signum(float f) {
        return Math.signum(f);
    }

    public static double copySign(double magnitude, double sign) {
        return Math.copySign(magnitude, (Double.isNaN(sign) ? 1.0d : sign));
    }

    public static float copySign(float magnitude, float sign) {
        return Math.copySign(magnitude, (Float.isNaN(sign) ? 1.0f : sign));
    }

    // ── ULP ─────────────────────────────────────────────────────────

    public static double ulp(double d) {
        return Math.ulp(d);
    }

    public static float ulp(float f) {
        return Math.ulp(f);
    }
}
