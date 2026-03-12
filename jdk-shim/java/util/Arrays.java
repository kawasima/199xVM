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

import java.util.stream.Stream;
import java.util.stream.StreamImpl;

public class Arrays {
    private Arrays() {}

    @SafeVarargs
    public static <T> List<T> asList(T... a) {
        ArrayList<T> list = new ArrayList<>();
        for (T e : a) {
            list.add(e);
        }
        return list;
    }

    public static byte[] copyOf(byte[] original, int newLength) {
        byte[] copy = new byte[newLength];
        int len = original.length < newLength ? original.length : newLength;
        for (int i = 0; i < len; i++) copy[i] = original[i];
        return copy;
    }

    public static byte[] copyOfRange(byte[] original, int from, int to) {
        int newLength = to - from;
        if (newLength < 0) throw new IllegalArgumentException();
        byte[] copy = new byte[newLength];
        int len = (original.length - from) < newLength ? (original.length - from) : newLength;
        for (int i = 0; i < len; i++) copy[i] = original[from + i];
        return copy;
    }

    public static int[] copyOf(int[] original, int newLength) {
        int[] copy = new int[newLength];
        int len = original.length < newLength ? original.length : newLength;
        for (int i = 0; i < len; i++) copy[i] = original[i];
        return copy;
    }

    public static long[] copyOf(long[] original, int newLength) {
        long[] copy = new long[newLength];
        int len = original.length < newLength ? original.length : newLength;
        for (int i = 0; i < len; i++) copy[i] = original[i];
        return copy;
    }

    public static int[] copyOfRange(int[] original, int from, int to) {
        int newLength = to - from;
        if (newLength < 0) throw new IllegalArgumentException();
        int[] copy = new int[newLength];
        int len = (original.length - from) < newLength ? (original.length - from) : newLength;
        for (int i = 0; i < len; i++) copy[i] = original[from + i];
        return copy;
    }

    public static long[] copyOfRange(long[] original, int from, int to) {
        int newLength = to - from;
        if (newLength < 0) throw new IllegalArgumentException();
        long[] copy = new long[newLength];
        int len = (original.length - from) < newLength ? (original.length - from) : newLength;
        for (int i = 0; i < len; i++) copy[i] = original[from + i];
        return copy;
    }

    @SuppressWarnings("unchecked")
    public static <T> T[] copyOf(T[] original, int newLength) {
        return (T[]) copyOf(original, newLength, original.getClass());
    }

    @SuppressWarnings("unchecked")
    public static <T, U> T[] copyOf(U[] original, int newLength, Class<? extends T[]> newType) {
        T[] copy = (T[]) java.lang.reflect.Array.newInstance(newType.getComponentType(), newLength);
        int len = original.length < newLength ? original.length : newLength;
        for (int i = 0; i < len; i++) copy[i] = (T) original[i];
        return copy;
    }

    @SuppressWarnings("unchecked")
    public static <T> T[] copyOfRange(T[] original, int from, int to) {
        int newLength = to - from;
        if (newLength < 0) throw new IllegalArgumentException();
        return (T[]) copyOfRange(original, from, to, original.getClass());
    }

    @SuppressWarnings("unchecked")
    public static <T, U> T[] copyOfRange(U[] original, int from, int to, Class<? extends T[]> newType) {
        int newLength = to - from;
        if (newLength < 0) throw new IllegalArgumentException();
        T[] copy = (T[]) java.lang.reflect.Array.newInstance(newType.getComponentType(), newLength);
        int len = (original.length - from) < newLength ? (original.length - from) : newLength;
        for (int i = 0; i < len; i++) copy[i] = (T) original[from + i];
        return copy;
    }

    public static void fill(byte[] a, byte val) {
        for (int i = 0; i < a.length; i++) a[i] = val;
    }

    public static void fill(int[] a, int val) {
        for (int i = 0; i < a.length; i++) a[i] = val;
    }

    public static void fill(long[] a, long val) {
        for (int i = 0; i < a.length; i++) a[i] = val;
    }

    public static void fill(int[] a, int fromIndex, int toIndex, int val) {
        for (int i = fromIndex; i < toIndex; i++) a[i] = val;
    }

    public static int hashCode(Object[] a) {
        if (a == null) return 0;
        int result = 1;
        for (int i = 0; i < a.length; i++) {
            Object e = a[i];
            result = 31 * result + (e == null ? 0 : e.hashCode());
        }
        return result;
    }

    public static int hashCode(int[] a) {
        if (a == null) return 0;
        int result = 1;
        for (int i = 0; i < a.length; i++) {
            result = 31 * result + a[i];
        }
        return result;
    }

    public static int hashCode(long[] a) {
        if (a == null) return 0;
        int result = 1;
        for (int i = 0; i < a.length; i++) {
            long e = a[i];
            result = 31 * result + (int) (e ^ (e >>> 32));
        }
        return result;
    }

    public static int hashCode(byte[] a) {
        if (a == null) return 0;
        int result = 1;
        for (int i = 0; i < a.length; i++) {
            result = 31 * result + a[i];
        }
        return result;
    }

    public static <T> Stream<T> stream(T[] array) {
        ArrayList<T> list = new ArrayList<>();
        if (array != null) {
            for (int i = 0; i < array.length; i++) {
                list.add(array[i]);
            }
        }
        return new StreamImpl<>(list);
    }
}
