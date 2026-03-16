/*
 * Copyright (c) 1994, 2025, Oracle and/or its affiliates. All rights reserved.
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
 * A thread-safe, mutable sequence of characters.
 * A string buffer is like a {@link String}, but can be modified. At any
 * point in time it contains some particular sequence of characters, but
 * the length and content of the sequence can be changed through certain
 * method calls.
 * <p>
 * String buffers are safe for use by multiple threads. The methods
 * are synchronized where necessary so that all the operations on any
 * particular instance behave as if they occur in some serial order
 * that is consistent with the order of the method calls made by each of
 * the individual threads involved.
 * <p>
 * As of release JDK 5, this class has been supplemented with an equivalent
 * class designed for use by a single thread, {@link StringBuilder}.  The
 * {@code StringBuilder} class should generally be used in preference to
 * this one, as it supports all of the same operations but it is faster, as
 * it performs no synchronization.
 *
 * @author      Arthur van Hoff
 * @see     java.lang.StringBuilder
 * @see     java.lang.String
 * @since   1.0
 */
public final class StringBuffer
    implements Comparable<StringBuffer>, CharSequence, Appendable
{
    private char[] value;
    private int count;

    public StringBuffer() {
        value = new char[16];
    }

    public StringBuffer(int capacity) {
        value = new char[capacity];
    }

    public StringBuffer(String str) {
        this();
        append(str);
    }

    public StringBuffer(CharSequence seq) {
        this(seq.length() + 16);
        append(seq);
    }

    @Override
    public synchronized int compareTo(StringBuffer another) {
        return CharSequence.compare(this, another);
    }

    public synchronized void ensureCapacity(int minimumCapacity) {
        if (minimumCapacity > value.length) {
            int newCapacity = value.length * 2 + 2;
            if (newCapacity < minimumCapacity) newCapacity = minimumCapacity;
            char[] newValue = new char[newCapacity];
            for (int i = 0; i < count; i++) {
                newValue[i] = value[i];
            }
            value = newValue;
        }
    }

    public synchronized int capacity() {
        return value.length;
    }

    public synchronized StringBuffer append(String str) {
        if (str == null) str = "null";
        int len = str.length();
        ensureCapacity(count + len);
        for (int i = 0; i < len; i++) {
            value[count++] = str.charAt(i);
        }
        return this;
    }

    public synchronized StringBuffer append(Object obj) {
        return append(String.valueOf(obj));
    }

    public synchronized StringBuffer append(StringBuffer sb) {
        if (sb == null) return append("null");
        int len = sb.length();
        ensureCapacity(count + len);
        for (int i = 0; i < len; i++) {
            value[count++] = sb.charAt(i);
        }
        return this;
    }

    public synchronized StringBuffer append(int i) {
        return append(Integer.toString(i));
    }

    public synchronized StringBuffer append(long l) {
        return append(Long.toString(l));
    }

    public synchronized StringBuffer append(float f) {
        return append(Float.toString(f));
    }

    public synchronized StringBuffer append(double d) {
        return append(Double.toString(d));
    }

    public synchronized StringBuffer append(boolean b) {
        return append(b ? "true" : "false");
    }

    public synchronized StringBuffer append(char c) {
        ensureCapacity(count + 1);
        value[count++] = c;
        return this;
    }

    @Override
    public synchronized StringBuffer append(CharSequence s) {
        if (s == null) s = "null";
        return append(s, 0, s.length());
    }

    @Override
    public synchronized StringBuffer append(CharSequence s, int start, int end) {
        if (s == null) s = "null";
        int len = end - start;
        ensureCapacity(count + len);
        for (int i = start; i < end; i++) {
            value[count++] = s.charAt(i);
        }
        return this;
    }

    public synchronized StringBuffer append(char[] str) {
        return append(str, 0, str.length);
    }

    public synchronized StringBuffer append(char[] str, int offset, int len) {
        ensureCapacity(count + len);
        for (int i = 0; i < len; i++) {
            value[count++] = str[offset + i];
        }
        return this;
    }

    public synchronized StringBuffer appendCodePoint(int codePoint) {
        if (Character.isBmpCodePoint(codePoint)) {
            return append((char) codePoint);
        }
        return append(Character.highSurrogate(codePoint))
               .append(Character.lowSurrogate(codePoint));
    }

    public synchronized StringBuffer delete(int start, int end) {
        if (end > count) end = count;
        int len = end - start;
        if (len > 0) {
            for (int i = start; i < count - len; i++) {
                value[i] = value[i + len];
            }
            count -= len;
        }
        return this;
    }

    public synchronized StringBuffer deleteCharAt(int index) {
        return delete(index, index + 1);
    }

    public synchronized StringBuffer replace(int start, int end, String str) {
        if (end > count) end = count;
        int newLen = str.length();
        int delta = newLen - (end - start);
        ensureCapacity(count + delta);
        if (delta != 0) {
            for (int i = count - 1; i >= end; i--) {
                value[i + delta] = value[i];
            }
        }
        for (int i = 0; i < newLen; i++) {
            value[start + i] = str.charAt(i);
        }
        count += delta;
        return this;
    }

    public synchronized StringBuffer insert(int offset, String str) {
        if (str == null) str = "null";
        int len = str.length();
        ensureCapacity(count + len);
        for (int i = count - 1; i >= offset; i--) {
            value[i + len] = value[i];
        }
        for (int i = 0; i < len; i++) {
            value[offset + i] = str.charAt(i);
        }
        count += len;
        return this;
    }

    public synchronized StringBuffer insert(int offset, char[] str, int off, int len) {
        ensureCapacity(count + len);
        for (int i = count - 1; i >= offset; i--) {
            value[i + len] = value[i];
        }
        for (int i = 0; i < len; i++) {
            value[offset + i] = str[off + i];
        }
        count += len;
        return this;
    }

    public synchronized StringBuffer insert(int offset, Object obj) {
        return insert(offset, String.valueOf(obj));
    }

    public synchronized StringBuffer insert(int offset, char c) {
        return insert(offset, String.valueOf(c));
    }

    public synchronized StringBuffer insert(int offset, int i) {
        return insert(offset, Integer.toString(i));
    }

    public synchronized StringBuffer insert(int offset, long l) {
        return insert(offset, Long.toString(l));
    }

    public synchronized StringBuffer insert(int offset, boolean b) {
        return insert(offset, String.valueOf(b));
    }

    public StringBuffer insert(int dstOffset, CharSequence s) {
        return insert(dstOffset, s.toString());
    }

    public synchronized StringBuffer insert(int dstOffset, CharSequence s, int start, int end) {
        String str = s.subSequence(start, end).toString();
        return insert(dstOffset, str);
    }

    public synchronized StringBuffer insert(int offset, float f) {
        return insert(offset, Float.toString(f));
    }

    public synchronized StringBuffer insert(int offset, double d) {
        return insert(offset, Double.toString(d));
    }

    public int indexOf(String str) {
        return indexOf(str, 0);
    }

    public synchronized int indexOf(String str, int fromIndex) {
        if (fromIndex < 0) fromIndex = 0;
        int strLen = str.length();
        if (strLen == 0) return fromIndex;
        int max = count - strLen;
        for (int i = fromIndex; i <= max; i++) {
            boolean found = true;
            for (int j = 0; j < strLen; j++) {
                if (value[i + j] != str.charAt(j)) {
                    found = false;
                    break;
                }
            }
            if (found) return i;
        }
        return -1;
    }

    public int lastIndexOf(String str) {
        return lastIndexOf(str, count);
    }

    public synchronized int lastIndexOf(String str, int fromIndex) {
        int strLen = str.length();
        int max = count - strLen;
        if (fromIndex > max) fromIndex = max;
        if (fromIndex < 0) return -1;
        if (strLen == 0) return fromIndex;
        for (int i = fromIndex; i >= 0; i--) {
            boolean found = true;
            for (int j = 0; j < strLen; j++) {
                if (value[i + j] != str.charAt(j)) {
                    found = false;
                    break;
                }
            }
            if (found) return i;
        }
        return -1;
    }

    public synchronized StringBuffer reverse() {
        int n = count;
        for (int i = 0; i < n / 2; i++) {
            char tmp = value[i];
            value[i] = value[n - 1 - i];
            value[n - 1 - i] = tmp;
        }
        return this;
    }

    public synchronized void setCharAt(int index, char ch) {
        value[index] = ch;
    }

    public synchronized void setLength(int newLength) {
        if (newLength < 0) {
            throw new StringIndexOutOfBoundsException(newLength);
        }
        ensureCapacity(newLength);
        if (newLength > count) {
            for (int i = count; i < newLength; i++) {
                value[i] = '\0';
            }
        }
        count = newLength;
    }

    public synchronized void trimToSize() {
        if (count < value.length) {
            char[] newValue = new char[count];
            for (int i = 0; i < count; i++) {
                newValue[i] = value[i];
            }
            value = newValue;
        }
    }

    public synchronized void getChars(int srcBegin, int srcEnd, char[] dst, int dstBegin) {
        if (srcBegin < 0 || srcEnd > count || srcBegin > srcEnd) {
            throw new StringIndexOutOfBoundsException();
        }
        for (int i = srcBegin; i < srcEnd; i++) {
            dst[dstBegin + (i - srcBegin)] = value[i];
        }
    }

    public synchronized String substring(int start) {
        return substring(start, count);
    }

    public synchronized String substring(int start, int end) {
        return new String(value, start, end - start);
    }

    public synchronized int codePointAt(int index) {
        return (int) value[index];
    }

    public synchronized int codePointBefore(int index) {
        return (int) value[index - 1];
    }

    public synchronized int codePointCount(int beginIndex, int endIndex) {
        return endIndex - beginIndex;
    }

    public synchronized int offsetByCodePoints(int index, int codePointOffset) {
        return index + codePointOffset;
    }

    @Override
    public synchronized int length() {
        return count;
    }

    @Override
    public synchronized char charAt(int index) {
        return value[index];
    }

    @Override
    public synchronized CharSequence subSequence(int start, int end) {
        return substring(start, end);
    }

    @Override
    public synchronized String toString() {
        return new String(value, 0, count);
    }
}
