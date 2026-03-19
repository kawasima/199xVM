/*
 * Copyright (c) 1995, 2024, Oracle and/or its affiliates. All rights reserved.
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

package java.util;

public class Random implements java.io.Serializable, java.util.random.RandomGenerator {
    private static final long serialVersionUID = 3905348978240129619L;
    private static final long multiplier = 0x5DEECE66DL;
    private static final long addend = 0xBL;
    private static final long mask = (1L << 48) - 1;

    private long seed;

    public Random() {
        this(System.currentTimeMillis() ^ multiplier);
    }

    public Random(long seed) {
        setSeed(seed);
    }

    public synchronized void setSeed(long seed) {
        this.seed = (seed ^ multiplier) & mask;
    }

    protected synchronized int next(int bits) {
        seed = (seed * multiplier + addend) & mask;
        return (int)(seed >>> (48 - bits));
    }

    public void nextBytes(byte[] bytes) {
        if (bytes == null) throw new NullPointerException();
        int i = 0;
        while (i < bytes.length) {
            int rnd = nextInt();
            for (int n = Math.min(bytes.length - i, 4); n-- > 0; rnd >>= 8) {
                bytes[i++] = (byte) rnd;
            }
        }
    }

    public int nextInt() {
        return next(32);
    }

    public int nextInt(int bound) {
        if (bound <= 0) throw new IllegalArgumentException("bound must be positive");

        if ((bound & -bound) == bound) {
            return (int)((bound * (long)next(31)) >> 31);
        }

        int bits;
        int value;
        do {
            bits = next(31);
            value = bits % bound;
        } while (bits - value + (bound - 1) < 0);
        return value;
    }

    public long nextLong() {
        return ((long)(next(32)) << 32) + next(32);
    }

    public boolean nextBoolean() {
        return next(1) != 0;
    }

    public float nextFloat() {
        return next(24) / ((float)(1 << 24));
    }

    public double nextDouble() {
        return ((((long)next(26)) << 27) + next(27)) / (double)(1L << 53);
    }
}
