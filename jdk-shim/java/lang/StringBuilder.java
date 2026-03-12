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

/**
 * StringBuilder for string concatenation and mutable character sequences.
 */
public final class StringBuilder implements CharSequence, Appendable {
    private char[] value;
    private int count;

    public StringBuilder() {
        value = new char[16];
    }

    public StringBuilder(int capacity) {
        value = new char[capacity];
    }

    public StringBuilder(String str) {
        this();
        append(str);
    }

    public StringBuilder(CharSequence seq) {
        this(seq.length() + 16);
        append(seq);
    }

    public void ensureCapacity(int minimumCapacity) {
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

    public int capacity() {
        return value.length;
    }

    public StringBuilder append(String str) {
        if (str == null) str = "null";
        int len = str.length();
        ensureCapacity(count + len);
        for (int i = 0; i < len; i++) {
            value[count++] = str.charAt(i);
        }
        return this;
    }

    public StringBuilder append(Object obj) {
        return append(String.valueOf(obj));
    }

    public StringBuilder append(int i) {
        return append(Integer.toString(i));
    }

    public StringBuilder append(long l) {
        return append(Long.toString(l));
    }

    public StringBuilder append(float f) {
        return append(Float.toString(f));
    }

    public StringBuilder append(double d) {
        return append(Double.toString(d));
    }

    public StringBuilder append(boolean b) {
        return append(b ? "true" : "false");
    }

    public StringBuilder append(char c) {
        ensureCapacity(count + 1);
        value[count++] = c;
        return this;
    }

    public StringBuilder append(CharSequence s) {
        if (s == null) s = "null";
        return append(s, 0, s.length());
    }

    public StringBuilder append(CharSequence s, int start, int end) {
        if (s == null) s = "null";
        int len = end - start;
        ensureCapacity(count + len);
        for (int i = start; i < end; i++) {
            value[count++] = s.charAt(i);
        }
        return this;
    }

    public StringBuilder append(char[] str) {
        return append(str, 0, str.length);
    }

    public StringBuilder append(char[] str, int offset, int len) {
        ensureCapacity(count + len);
        for (int i = 0; i < len; i++) {
            value[count++] = str[offset + i];
        }
        return this;
    }

    public StringBuilder delete(int start, int end) {
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

    public StringBuilder deleteCharAt(int index) {
        return delete(index, index + 1);
    }

    public StringBuilder replace(int start, int end, String str) {
        if (end > count) end = count;
        int newLen = str.length();
        int delta = newLen - (end - start);
        ensureCapacity(count + delta);
        // shift tail
        if (delta != 0) {
            for (int i = count - 1; i >= end; i--) {
                value[i + delta] = value[i];
            }
        }
        // copy replacement
        for (int i = 0; i < newLen; i++) {
            value[start + i] = str.charAt(i);
        }
        count += delta;
        return this;
    }

    public StringBuilder insert(int offset, String str) {
        if (str == null) str = "null";
        int len = str.length();
        ensureCapacity(count + len);
        // shift right
        for (int i = count - 1; i >= offset; i--) {
            value[i + len] = value[i];
        }
        for (int i = 0; i < len; i++) {
            value[offset + i] = str.charAt(i);
        }
        count += len;
        return this;
    }

    public StringBuilder insert(int offset, char c) {
        return insert(offset, String.valueOf(c));
    }

    public StringBuilder insert(int offset, int i) {
        return insert(offset, Integer.toString(i));
    }

    public StringBuilder insert(int offset, long l) {
        return insert(offset, Long.toString(l));
    }

    public StringBuilder insert(int offset, boolean b) {
        return insert(offset, String.valueOf(b));
    }

    public StringBuilder insert(int offset, Object obj) {
        return insert(offset, String.valueOf(obj));
    }

    public int indexOf(String str) {
        return indexOf(str, 0);
    }

    public int indexOf(String str, int fromIndex) {
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

    public int lastIndexOf(String str, int fromIndex) {
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

    public StringBuilder reverse() {
        int n = count;
        for (int i = 0; i < n / 2; i++) {
            char tmp = value[i];
            value[i] = value[n - 1 - i];
            value[n - 1 - i] = tmp;
        }
        return this;
    }

    public void setCharAt(int index, char ch) {
        value[index] = ch;
    }

    public void setLength(int newLength) {
        if (newLength < 0) {
            throw new StringIndexOutOfBoundsException(newLength);
        }
        ensureCapacity(newLength);
        if (newLength > count) {
            // pad with null chars
            for (int i = count; i < newLength; i++) {
                value[i] = '\0';
            }
        }
        count = newLength;
    }

    public String substring(int start) {
        return substring(start, count);
    }

    public String substring(int start, int end) {
        return new String(value, start, end - start);
    }

    public int codePointAt(int index) {
        return (int) value[index];
    }

    public int codePointCount(int beginIndex, int endIndex) {
        // Simple implementation: each char is one code point
        // (does not handle surrogate pairs)
        return endIndex - beginIndex;
    }

    public StringBuilder repeat(char c, int count) {
        if (count < 0) throw new IllegalArgumentException("count < 0");
        ensureCapacity(this.count + count);
        for (int i = 0; i < count; i++) {
            value[this.count++] = c;
        }
        return this;
    }

    @Override
    public int length() {
        return count;
    }

    @Override
    public char charAt(int index) {
        return value[index];
    }

    @Override
    public CharSequence subSequence(int start, int end) {
        return substring(start, end);
    }

    @Override
    public String toString() {
        return new String(value, 0, count);
    }
}
