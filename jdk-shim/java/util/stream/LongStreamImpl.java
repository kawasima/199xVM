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

package java.util.stream;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.NoSuchElementException;
import java.util.OptionalDouble;
import java.util.OptionalLong;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.function.BiConsumer;
import java.util.function.LongBinaryOperator;
import java.util.function.LongConsumer;
import java.util.function.LongFunction;
import java.util.function.LongPredicate;
import java.util.function.LongToDoubleFunction;
import java.util.function.LongToIntFunction;
import java.util.function.LongUnaryOperator;
import java.util.function.ObjLongConsumer;
import java.util.function.Supplier;

public class LongStreamImpl implements LongStream {
    private final long[] elements;
    private boolean parallel;
    private List<Runnable> closeHandlers;

    public LongStreamImpl(long[] elements) {
        this.elements = elements;
    }

    @Override
    public LongStream filter(LongPredicate predicate) {
        int count = 0;
        for (long e : elements) {
            if (predicate.test(e)) count++;
        }
        long[] result = new long[count];
        int idx = 0;
        for (long e : elements) {
            if (predicate.test(e)) result[idx++] = e;
        }
        return new LongStreamImpl(result);
    }

    @Override
    public LongStream map(LongUnaryOperator mapper) {
        long[] result = new long[elements.length];
        for (int i = 0; i < elements.length; i++) {
            result[i] = mapper.applyAsLong(elements[i]);
        }
        return new LongStreamImpl(result);
    }

    @Override
    public <U> Stream<U> mapToObj(LongFunction<? extends U> mapper) {
        List<U> result = new ArrayList<>();
        for (long e : elements) {
            result.add(mapper.apply(e));
        }
        return new StreamImpl<>(result);
    }

    @Override
    public IntStream mapToInt(LongToIntFunction mapper) {
        int[] result = new int[elements.length];
        for (int i = 0; i < elements.length; i++) {
            result[i] = mapper.applyAsInt(elements[i]);
        }
        return new IntStreamImpl(result);
    }

    @Override
    public DoubleStream mapToDouble(LongToDoubleFunction mapper) {
        double[] result = new double[elements.length];
        for (int i = 0; i < elements.length; i++) {
            result[i] = mapper.applyAsDouble(elements[i]);
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public LongStream flatMap(LongFunction<? extends LongStream> mapper) {
        long[] buf = new long[elements.length * 4];
        int size = 0;
        for (long e : elements) {
            LongStream s = mapper.apply(e);
            if (s != null) {
                try {
                    long[] arr = s.toArray();
                    while (size + arr.length > buf.length) {
                        buf = Arrays.copyOf(buf, buf.length * 2);
                    }
                    System.arraycopy(arr, 0, buf, size, arr.length);
                    size += arr.length;
                } finally {
                    s.close();
                }
            }
        }
        return new LongStreamImpl(Arrays.copyOf(buf, size));
    }

    @Override
    public LongStream distinct() {
        java.util.LinkedHashSet<Long> seen = new java.util.LinkedHashSet<>();
        for (long e : elements) {
            seen.add(e);
        }
        long[] result = new long[seen.size()];
        int idx = 0;
        for (long v : seen) {
            result[idx++] = v;
        }
        return new LongStreamImpl(result);
    }

    @Override
    public LongStream sorted() {
        long[] result = Arrays.copyOf(elements, elements.length);
        Arrays.sort(result);
        return new LongStreamImpl(result);
    }

    @Override
    public LongStream peek(LongConsumer action) {
        long[] result = new long[elements.length];
        for (int i = 0; i < elements.length; i++) {
            action.accept(elements[i]);
            result[i] = elements[i];
        }
        return new LongStreamImpl(result);
    }

    @Override
    public LongStream limit(long maxSize) {
        if (maxSize < 0) throw new IllegalArgumentException(Long.toString(maxSize));
        int len = (int) Math.min(elements.length, maxSize);
        return new LongStreamImpl(Arrays.copyOf(elements, len));
    }

    @Override
    public LongStream skip(long n) {
        if (n < 0) throw new IllegalArgumentException(Long.toString(n));
        int skip = (int) Math.min(elements.length, n);
        return new LongStreamImpl(Arrays.copyOfRange(elements, skip, elements.length));
    }

    @Override
    public void forEach(LongConsumer action) {
        for (long e : elements) {
            action.accept(e);
        }
    }

    @Override
    public void forEachOrdered(LongConsumer action) {
        forEach(action);
    }

    @Override
    public long[] toArray() {
        return Arrays.copyOf(elements, elements.length);
    }

    @Override
    public long reduce(long identity, LongBinaryOperator op) {
        long result = identity;
        for (long e : elements) {
            result = op.applyAsLong(result, e);
        }
        return result;
    }

    @Override
    public OptionalLong reduce(LongBinaryOperator op) {
        if (elements.length == 0) return OptionalLong.empty();
        long result = elements[0];
        for (int i = 1; i < elements.length; i++) {
            result = op.applyAsLong(result, elements[i]);
        }
        return OptionalLong.of(result);
    }

    @Override
    public <R> R collect(Supplier<R> supplier,
                         ObjLongConsumer<R> accumulator,
                         BiConsumer<R, R> combiner) {
        R result = supplier.get();
        for (long e : elements) {
            accumulator.accept(result, e);
        }
        return result;
    }

    @Override
    public long sum() {
        long sum = 0;
        for (long e : elements) {
            sum += e;
        }
        return sum;
    }

    @Override
    public OptionalLong min() {
        if (elements.length == 0) return OptionalLong.empty();
        long min = elements[0];
        for (int i = 1; i < elements.length; i++) {
            if (elements[i] < min) min = elements[i];
        }
        return OptionalLong.of(min);
    }

    @Override
    public OptionalLong max() {
        if (elements.length == 0) return OptionalLong.empty();
        long max = elements[0];
        for (int i = 1; i < elements.length; i++) {
            if (elements[i] > max) max = elements[i];
        }
        return OptionalLong.of(max);
    }

    @Override
    public long count() {
        return elements.length;
    }

    @Override
    public OptionalDouble average() {
        if (elements.length == 0) return OptionalDouble.empty();
        long sum = 0;
        for (long e : elements) {
            sum += e;
        }
        return OptionalDouble.of((double) sum / elements.length);
    }

    @Override
    public java.util.LongSummaryStatistics summaryStatistics() {
        throw new UnsupportedOperationException("summaryStatistics");
    }

    @Override
    public boolean anyMatch(LongPredicate predicate) {
        for (long e : elements) {
            if (predicate.test(e)) return true;
        }
        return false;
    }

    @Override
    public boolean allMatch(LongPredicate predicate) {
        for (long e : elements) {
            if (!predicate.test(e)) return false;
        }
        return true;
    }

    @Override
    public boolean noneMatch(LongPredicate predicate) {
        for (long e : elements) {
            if (predicate.test(e)) return false;
        }
        return true;
    }

    @Override
    public OptionalLong findFirst() {
        if (elements.length == 0) return OptionalLong.empty();
        return OptionalLong.of(elements[0]);
    }

    @Override
    public OptionalLong findAny() {
        if (elements.length == 0) return OptionalLong.empty();
        return OptionalLong.of(elements[0]);
    }

    @Override
    public DoubleStream asDoubleStream() {
        double[] result = new double[elements.length];
        for (int i = 0; i < elements.length; i++) {
            result[i] = elements[i];
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public Stream<Long> boxed() {
        List<Long> result = new ArrayList<>();
        for (long e : elements) {
            result.add(e);
        }
        return new StreamImpl<>(result);
    }

    // BaseStream methods

    @Override
    public java.util.PrimitiveIterator.OfLong iterator() {
        return new java.util.PrimitiveIterator.OfLong() {
            private int index = 0;

            @Override
            public boolean hasNext() {
                return index < elements.length;
            }

            @Override
            public long nextLong() {
                if (!hasNext()) throw new NoSuchElementException();
                return elements[index++];
            }
        };
    }

    @Override
    public Spliterator.OfLong spliterator() {
        return Spliterators.spliterator(elements, 0, elements.length,
                Spliterator.ORDERED | Spliterator.SIZED | Spliterator.SUBSIZED);
    }

    @Override
    public boolean isParallel() {
        return parallel;
    }

    @Override
    public LongStream sequential() {
        this.parallel = false;
        return this;
    }

    @Override
    public LongStream parallel() {
        this.parallel = true;
        return this;
    }

    @Override
    public LongStream unordered() {
        return this;
    }

    @Override
    public LongStream onClose(Runnable closeHandler) {
        if (this.closeHandlers == null) {
            this.closeHandlers = new ArrayList<>();
        }
        this.closeHandlers.add(closeHandler);
        return this;
    }

    @Override
    public void close() {
        if (closeHandlers != null) {
            List<Runnable> handlers = closeHandlers;
            closeHandlers = null;
            Throwable first = null;
            for (Runnable h : handlers) {
                try {
                    h.run();
                } catch (Throwable t) {
                    if (first == null) {
                        first = t;
                    }
                }
            }
            if (first instanceof RuntimeException) {
                throw (RuntimeException) first;
            } else if (first instanceof Error) {
                throw (Error) first;
            } else if (first != null) {
                throw new RuntimeException(first);
            }
        }
    }
}
