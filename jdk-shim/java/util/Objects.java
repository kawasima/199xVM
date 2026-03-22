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

package java.util;

import java.util.function.Supplier;

public final class Objects {
    private Objects() {}

    public static <T> T requireNonNull(T obj) {
        if (obj == null) throw new NullPointerException();
        return obj;
    }

    public static <T> T requireNonNull(T obj, String message) {
        if (obj == null) throw new NullPointerException(message);
        return obj;
    }

    public static int checkIndex(int index, int length) {
        if (index < 0 || index >= length) throw new IndexOutOfBoundsException();
        return index;
    }

    public static int checkFromToIndex(int fromIndex, int toIndex, int length) {
        if (fromIndex < 0 || toIndex < fromIndex || toIndex > length) throw new IndexOutOfBoundsException();
        return fromIndex;
    }

    public static int checkFromIndexSize(int fromIndex, int size, int length) {
        if (fromIndex < 0 || size < 0 || fromIndex + size > length) throw new IndexOutOfBoundsException();
        return fromIndex;
    }

    public static <T> T requireNonNullElse(T obj, T defaultObj) {
        return (obj != null) ? obj : requireNonNull(defaultObj, "defaultObj");
    }

    public static <T> T requireNonNullElseGet(T obj, Supplier<? extends T> supplier) {
        if (obj != null) return obj;
        return requireNonNull(supplier.get(), "supplier.get()");
    }

    public static boolean isNull(Object obj) {
        return obj == null;
    }

    public static boolean nonNull(Object obj) {
        return obj != null;
    }

    public static boolean equals(Object a, Object b) {
        return (a == b) || (a != null && a.equals(b));
    }

    public static int hashCode(Object o) {
        return o != null ? o.hashCode() : 0;
    }

    public static int hash(Object... values) {
        if (values == null) return 0;
        int result = 1;
        for (Object element : values) {
            result = 31 * result + (element == null ? 0 : element.hashCode());
        }
        return result;
    }

    public static String toString(Object o) {
        return String.valueOf(o);
    }

    public static String toString(Object o, String nullDefault) {
        return (o != null) ? o.toString() : nullDefault;
    }
}
