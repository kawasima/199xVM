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

public final class Short extends Number implements Comparable<Short> {
    public static final int SIZE = 16;
    public static final int BYTES = 2;
    @SuppressWarnings("unchecked")
    public static final Class<Short> TYPE = (Class<Short>) primitiveType("short");
    private static Class<?> primitiveType(String name) { try { return Class.forName(name); } catch (ClassNotFoundException e) { return null; } }
    private final short value;
    public Short(short value) { this.value = value; }
    public static Short valueOf(short s) { return new Short(s); }
    @Override public int intValue() { return value; }
    @Override public long longValue() { return value; }
    @Override public float floatValue() { return value; }
    @Override public double doubleValue() { return value; }
    @Override public int compareTo(Short another) { return value - another.value; }
    @Override public String toString() { return Integer.toString(value); }
}
