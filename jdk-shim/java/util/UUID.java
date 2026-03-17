/*
 * Copyright (c) 2003, 2025, Oracle and/or its affiliates. All rights reserved.
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

import java.io.Serializable;

public final class UUID implements Serializable, Comparable<UUID> {
    private static final long serialVersionUID = -4856846361193249489L;

    private final long mostSigBits;
    private final long leastSigBits;

    public UUID(long mostSigBits, long leastSigBits) {
        this.mostSigBits = mostSigBits;
        this.leastSigBits = leastSigBits;
    }

    public static UUID randomUUID() {
        long now = System.nanoTime();
        long mix = (System.currentTimeMillis() << 32) ^ now ^ 0x9e3779b97f4a7c15L;
        long msb = now ^ Long.rotateLeft(mix, 17);
        long lsb = mix ^ Long.rotateLeft(now, 29);
        msb &= 0xffffffffffff0fffL;
        msb |= 0x0000000000004000L;
        lsb &= 0x3fffffffffffffffL;
        lsb |= 0x8000000000000000L;
        return new UUID(msb, lsb);
    }

    public static UUID fromString(String name) {
        if (name == null) {
            throw new NullPointerException("name");
        }
        String[] parts = name.split("-");
        if (parts.length != 5) {
            throw new IllegalArgumentException("Invalid UUID string: " + name);
        }
        long part0 = parseHex(parts[0], 8);
        long part1 = parseHex(parts[1], 4);
        long part2 = parseHex(parts[2], 4);
        long part3 = parseHex(parts[3], 4);
        long part4 = parseHex(parts[4], 12);
        long msb = (part0 << 32) | (part1 << 16) | part2;
        long lsb = (part3 << 48) | part4;
        return new UUID(msb, lsb);
    }

    public long getMostSignificantBits() {
        return mostSigBits;
    }

    public long getLeastSignificantBits() {
        return leastSigBits;
    }

    public int version() {
        return (int)((mostSigBits >>> 12) & 0x0f);
    }

    public int variant() {
        return (int)((leastSigBits >>> 62) & 0x03);
    }

    public int hashCode() {
        long hilo = mostSigBits ^ leastSigBits;
        return (int)(hilo >> 32) ^ (int)hilo;
    }

    public boolean equals(Object obj) {
        if (!(obj instanceof UUID other)) {
            return false;
        }
        return mostSigBits == other.mostSigBits && leastSigBits == other.leastSigBits;
    }

    public int compareTo(UUID other) {
        int cmp = Long.compare(mostSigBits, other.mostSigBits);
        if (cmp != 0) {
            return cmp;
        }
        return Long.compare(leastSigBits, other.leastSigBits);
    }

    public String toString() {
        return digits(mostSigBits >>> 32, 8) + "-"
            + digits(mostSigBits >>> 16, 4) + "-"
            + digits(mostSigBits, 4) + "-"
            + digits(leastSigBits >>> 48, 4) + "-"
            + digits(leastSigBits, 12);
    }

    private static long parseHex(String value, int digits) {
        if (value.length() != digits) {
            throw new IllegalArgumentException("Invalid UUID string");
        }
        return Long.parseLong(value, 16);
    }

    private static String digits(long value, int digits) {
        long mask = digits == 16 ? -1L : (1L << (digits * 4)) - 1L;
        String text = Long.toHexString(value & mask);
        if (text.length() >= digits) {
            return text.substring(text.length() - digits);
        }
        StringBuilder sb = new StringBuilder(digits);
        for (int i = text.length(); i < digits; i++) {
            sb.append('0');
        }
        sb.append(text);
        return sb.toString();
    }
}
