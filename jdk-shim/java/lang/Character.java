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

public final class Character implements Serializable, Comparable<Character> {
    public static final int MIN_RADIX = 2;
    public static final int MAX_RADIX = 36;
    public static final int MIN_SUPPLEMENTARY_CODE_POINT = 0x10000;
    public static final int MAX_CODE_POINT = 0x10FFFF;
    public static final byte NON_SPACING_MARK = 6;
    public static final byte SPACE_SEPARATOR = 12;
    @SuppressWarnings("unchecked")
    public static final Class<Character> TYPE = (Class<Character>) primitiveType("char");
    private static Class<?> primitiveType(String name) { try { return Class.forName(name); } catch (ClassNotFoundException e) { return null; } }
    private final char value;
    public Character(char value) { this.value = value; }
    public static Character valueOf(char c) { return new Character(c); }
    public char charValue() { return value; }
    @Override public int compareTo(Character another) { return value - another.value; }
    @Override public String toString() { return String.valueOf(value); }
    @Override public int hashCode() { return (int) value; }
    @Override public boolean equals(Object obj) { return obj instanceof Character && ((Character) obj).value == value; }

    public static int digit(char ch, int radix) {
        int val;
        if (ch >= '0' && ch <= '9') val = ch - '0';
        else if (ch >= 'a' && ch <= 'z') val = ch - 'a' + 10;
        else if (ch >= 'A' && ch <= 'Z') val = ch - 'A' + 10;
        else return -1;
        return (val < radix) ? val : -1;
    }

    public static char forDigit(int digit, int radix) {
        if (digit < 0 || digit >= radix || radix < 2 || radix > 36) return '\0';
        if (digit < 10) return (char) ('0' + digit);
        return (char) ('a' + digit - 10);
    }

    public static boolean isDigit(char ch) {
        return ch >= '0' && ch <= '9';
    }

    public static boolean isLetter(char ch) {
        return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z');
    }

    public static boolean isLetterOrDigit(char ch) {
        return isLetter(ch) || isDigit(ch);
    }

    public static boolean isUpperCase(char ch) {
        return ch >= 'A' && ch <= 'Z';
    }

    public static boolean isLowerCase(char ch) {
        return ch >= 'a' && ch <= 'z';
    }

    public static char toUpperCase(char ch) {
        return isLowerCase(ch) ? (char)(ch - 32) : ch;
    }

    public static char toLowerCase(char ch) {
        return isUpperCase(ch) ? (char)(ch + 32) : ch;
    }
    public static int toUpperCase(int ch) { return toUpperCase((char) ch); }
    public static int toLowerCase(int ch) { return toLowerCase((char) ch); }

    public static boolean isWhitespace(char ch) {
        return ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' || ch == '\f';
    }

    public static boolean isJavaIdentifierStart(char ch) {
        return isLetter(ch) || ch == '_' || ch == '$';
    }

    public static boolean isJavaIdentifierPart(char ch) {
        return isJavaIdentifierStart(ch) || isDigit(ch);
    }

    public static int getType(int codePoint) {
        if (codePoint == ' ') return SPACE_SEPARATOR;
        return 0;
    }
    public static int getType(char ch) { return getType((int) ch); }

    public static int charCount(int codePoint) {
        return isSupplementaryCodePoint(codePoint) ? 2 : 1;
    }

    public static boolean isSupplementaryCodePoint(int codePoint) {
        return codePoint >= MIN_SUPPLEMENTARY_CODE_POINT && codePoint <= MAX_CODE_POINT;
    }

    public static boolean isHighSurrogate(char ch) {
        return ch >= 0xD800 && ch <= 0xDBFF;
    }

    public static boolean isLowSurrogate(char ch) {
        return ch >= 0xDC00 && ch <= 0xDFFF;
    }

    public static boolean isSurrogate(char ch) {
        return isHighSurrogate(ch) || isLowSurrogate(ch);
    }

    public static boolean isSurrogatePair(char high, char low) {
        return isHighSurrogate(high) && isLowSurrogate(low);
    }

    public static int toCodePoint(char high, char low) {
        return (((high - 0xD800) << 10) | (low - 0xDC00)) + 0x10000;
    }

    public static int codePointAt(CharSequence seq, int index) {
        char c1 = seq.charAt(index);
        if (index + 1 < seq.length()) {
            char c2 = seq.charAt(index + 1);
            if (isSurrogatePair(c1, c2)) return toCodePoint(c1, c2);
        }
        return c1;
    }
    public static int codePointAt(String seq, int index) { return codePointAt((CharSequence) seq, index); }

    public static int codePointBefore(CharSequence seq, int index) {
        char c1 = seq.charAt(index - 1);
        if (index - 2 >= 0) {
            char c0 = seq.charAt(index - 2);
            if (isSurrogatePair(c0, c1)) return toCodePoint(c0, c1);
        }
        return c1;
    }

    public static int compare(char x, char y) {
        return x - y;
    }

    public static int codePointCount(CharSequence seq, int beginIndex, int endIndex) {
        int count = 0;
        for (int i = beginIndex; i < endIndex; count++) {
            int cp = codePointAt(seq, i);
            i += charCount(cp);
        }
        return count;
    }
}
