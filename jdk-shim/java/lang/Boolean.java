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

import java.io.Serializable;

public final class Boolean implements Serializable, Comparable<Boolean> {
    public static final Boolean TRUE = new Boolean(true);
    public static final Boolean FALSE = new Boolean(false);
    @SuppressWarnings("unchecked")
    public static final Class<Boolean> TYPE = (Class<Boolean>) primitiveType("boolean");
    private static Class<?> primitiveType(String name) { try { return Class.forName(name); } catch (ClassNotFoundException e) { return null; } }

    private final boolean value;

    public Boolean(boolean value) {
        this.value = value;
    }

    public static Boolean valueOf(boolean b) {
        return b ? TRUE : FALSE;
    }

    public boolean booleanValue() {
        return value;
    }

    @Override
    public String toString() {
        return value ? "true" : "false";
    }

    @Override
    public int hashCode() {
        return value ? 1231 : 1237;
    }

    @Override
    public boolean equals(Object obj) {
        if (obj instanceof Boolean other) {
            return value == other.value;
        }
        return false;
    }

    @Override
    public int compareTo(Boolean b) {
        return compare(this.value, b.value);
    }

    public static int compare(boolean x, boolean y) {
        return (x == y) ? 0 : (x ? 1 : -1);
    }
}
