/*
 * Copyright (c) 2000, 2025, Oracle and/or its affiliates. All rights reserved.
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

import java.util.NoSuchElementException;
import java.util.Objects;
import java.util.PrimitiveIterator;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.function.IntConsumer;
import java.util.stream.IntStream;
import java.util.stream.StreamSupport;

/**
 * A {@code CharSequence} is a readable sequence of {@code char} values. This
 * interface provides uniform, read-only access to many different kinds of
 * {@code char} sequences.
 * A {@code char} value represents a character in the <i>Basic
 * Multilingual Plane (BMP)</i> or a surrogate. Refer to <a
 * href="Character.html#unicode">Unicode Character Representation</a> for details.
 *
 * @author Mike McCloskey
 * @since 1.4
 */
public interface CharSequence {

    int length();

    char charAt(int index);

    /**
     * Returns {@code true} if this character sequence is empty.
     *
     * @return {@code true} if {@link #length()} is {@code 0}, otherwise
     * {@code false}
     *
     * @since 15
     */
    default boolean isEmpty() {
        return this.length() == 0;
    }

    CharSequence subSequence(int start, int end);

    String toString();

    /**
     * Returns a stream of {@code int} zero-extending the {@code char} values
     * from this sequence.
     *
     * @return an IntStream of char values from this sequence
     * @since 1.8
     */
    public default IntStream chars() {
        class CharIterator implements PrimitiveIterator.OfInt {
            int cur = 0;

            public boolean hasNext() {
                return cur < length();
            }

            public int nextInt() {
                if (hasNext()) {
                    return charAt(cur++);
                } else {
                    throw new NoSuchElementException();
                }
            }

            @Override
            public void forEachRemaining(IntConsumer block) {
                for (; cur < length(); cur++) {
                    block.accept(charAt(cur));
                }
            }
        }

        return StreamSupport.intStream(() ->
                Spliterators.spliterator(
                        new CharIterator(),
                        length(),
                        Spliterator.ORDERED),
                Spliterator.SUBSIZED | Spliterator.SIZED | Spliterator.ORDERED,
                false);
    }

    /**
     * Returns a stream of code point values from this sequence. Any surrogate
     * pairs encountered in the sequence are combined as if by {@linkplain
     * Character#toCodePoint Character.toCodePoint} and the result is passed
     * to the stream.
     *
     * @return an IntStream of Unicode code points from this sequence
     * @since 1.8
     */
    public default IntStream codePoints() {
        class CodePointIterator implements PrimitiveIterator.OfInt {
            int cur = 0;

            @Override
            public void forEachRemaining(IntConsumer block) {
                final int length = length();
                int i = cur;
                try {
                    while (i < length) {
                        char c1 = charAt(i++);
                        if (!Character.isHighSurrogate(c1) || i >= length) {
                            block.accept(c1);
                        } else {
                            char c2 = charAt(i);
                            if (Character.isLowSurrogate(c2)) {
                                i++;
                                block.accept(Character.toCodePoint(c1, c2));
                            } else {
                                block.accept(c1);
                            }
                        }
                    }
                } finally {
                    cur = i;
                }
            }

            public boolean hasNext() {
                return cur < length();
            }

            public int nextInt() {
                final int length = length();

                if (cur >= length) {
                    throw new NoSuchElementException();
                }
                char c1 = charAt(cur++);
                if (Character.isHighSurrogate(c1) && cur < length) {
                    char c2 = charAt(cur);
                    if (Character.isLowSurrogate(c2)) {
                        cur++;
                        return Character.toCodePoint(c1, c2);
                    }
                }
                return c1;
            }
        }

        return StreamSupport.intStream(() ->
                Spliterators.spliteratorUnknownSize(
                        new CodePointIterator(),
                        Spliterator.ORDERED),
                Spliterator.ORDERED,
                false);
    }

    /**
     * Compares two {@code CharSequence} instances lexicographically.
     *
     * @param cs1 the first {@code CharSequence}
     * @param cs2 the second {@code CharSequence}
     *
     * @return  the value {@code 0} if the two {@code CharSequence} are equal;
     *          a negative integer if the first {@code CharSequence}
     *          is lexicographically less than the second; or a
     *          positive integer if the first {@code CharSequence} is
     *          lexicographically greater than the second.
     *
     * @since 11
     */
    @SuppressWarnings("unchecked")
    public static int compare(CharSequence cs1, CharSequence cs2) {
        if (Objects.requireNonNull(cs1) == Objects.requireNonNull(cs2)) {
            return 0;
        }

        if (cs1.getClass() == cs2.getClass() && cs1 instanceof Comparable) {
            return ((Comparable<Object>) cs1).compareTo(cs2);
        }

        for (int i = 0, len = Math.min(cs1.length(), cs2.length()); i < len; i++) {
            char a = cs1.charAt(i);
            char b = cs2.charAt(i);
            if (a != b) {
                return a - b;
            }
        }

        return cs1.length() - cs2.length();
    }

    /**
     * Copies characters from this sequence into the given destination array.
     *
     * @param      srcBegin   start copying at this offset.
     * @param      srcEnd     stop copying at this offset.
     * @param      dst        the array to copy the data into.
     * @param      dstBegin   offset into {@code dst}.
     * @throws     IndexOutOfBoundsException  if any of the following is true:
     *             <ul>
     *             <li>{@code srcBegin} is negative
     *             <li>{@code dstBegin} is negative
     *             <li>the {@code srcBegin} argument is greater than
     *             the {@code srcEnd} argument.
     *             <li>{@code srcEnd} is greater than
     *             {@code this.length()}.
     *             <li>{@code dstBegin+srcEnd-srcBegin} is greater than
     *             {@code dst.length}
     *             </ul>
     *
     * @since 25
     */
    public default void getChars(int srcBegin, int srcEnd, char[] dst, int dstBegin) {
        Objects.checkFromToIndex(srcBegin, srcEnd, length());
        Objects.checkFromToIndex(dstBegin, dstBegin + (srcEnd - srcBegin), dst.length);
        while (srcBegin < srcEnd)
            dst[dstBegin++] = charAt(srcBegin++);
    }
}
