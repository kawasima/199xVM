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
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;
import java.util.OptionalDouble;
import java.util.OptionalInt;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.function.BiConsumer;
import java.util.function.IntBinaryOperator;
import java.util.function.IntConsumer;
import java.util.function.IntFunction;
import java.util.function.IntPredicate;
import java.util.function.IntToDoubleFunction;
import java.util.function.IntToLongFunction;
import java.util.function.IntUnaryOperator;
import java.util.function.ObjIntConsumer;
import java.util.function.Supplier;

public class IntStreamImpl implements IntStream {
    private int[] elements;
    private final Spliterator.OfInt lazySpliterator;
    private boolean parallel;
    private List<Runnable> closeHandlers;

    public IntStreamImpl(int[] elements) {
        this.elements = elements;
        this.lazySpliterator = null;
    }

    /** Lazy constructor: spliterator is not traversed until a terminal operation. */
    public IntStreamImpl(Spliterator.OfInt spliterator, boolean parallel) {
        this.elements = null;
        this.lazySpliterator = spliterator;
        this.parallel = parallel;
    }

    private int[] materialize() {
        if (elements != null) return elements;
        List<Integer> list = new ArrayList<>();
        lazySpliterator.forEachRemaining((IntConsumer) (int v) -> list.add(v));
        int[] arr = new int[list.size()];
        for (int i = 0; i < arr.length; i++) arr[i] = list.get(i);
        elements = arr;
        return arr;
    }

    @Override
    public IntStream filter(IntPredicate predicate) {
        int count = 0;
        for (int e : materialize()) {
            if (predicate.test(e)) count++;
        }
        int[] result = new int[count];
        int idx = 0;
        for (int e : materialize()) {
            if (predicate.test(e)) result[idx++] = e;
        }
        return new IntStreamImpl(result);
    }

    @Override
    public IntStream map(IntUnaryOperator mapper) {
        int[] result = new int[materialize().length];
        for (int i = 0; i < materialize().length; i++) {
            result[i] = mapper.applyAsInt(materialize()[i]);
        }
        return new IntStreamImpl(result);
    }

    @Override
    public <U> Stream<U> mapToObj(IntFunction<? extends U> mapper) {
        List<U> result = new ArrayList<>();
        for (int e : materialize()) {
            result.add(mapper.apply(e));
        }
        return new StreamImpl<>(result);
    }

    @Override
    public LongStream mapToLong(IntToLongFunction mapper) {
        long[] result = new long[materialize().length];
        for (int i = 0; i < materialize().length; i++) {
            result[i] = mapper.applyAsLong(materialize()[i]);
        }
        return new LongStreamImpl(result);
    }

    @Override
    public DoubleStream mapToDouble(IntToDoubleFunction mapper) {
        double[] result = new double[materialize().length];
        for (int i = 0; i < materialize().length; i++) {
            result[i] = mapper.applyAsDouble(materialize()[i]);
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public IntStream flatMap(IntFunction<? extends IntStream> mapper) {
        // Collect all results eagerly
        int[] buf = new int[materialize().length * 4];
        int size = 0;
        for (int e : materialize()) {
            IntStream s = mapper.apply(e);
            if (s != null) {
                try {
                    int[] arr = s.toArray();
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
        return new IntStreamImpl(Arrays.copyOf(buf, size));
    }

    @Override
    public IntStream distinct() {
        java.util.LinkedHashSet<Integer> seen = new java.util.LinkedHashSet<>();
        for (int e : materialize()) {
            seen.add(e);
        }
        int[] result = new int[seen.size()];
        int idx = 0;
        for (int v : seen) {
            result[idx++] = v;
        }
        return new IntStreamImpl(result);
    }

    @Override
    public IntStream sorted() {
        int[] result = Arrays.copyOf(materialize(), materialize().length);
        Arrays.sort(result);
        return new IntStreamImpl(result);
    }

    @Override
    public IntStream peek(IntConsumer action) {
        int[] result = new int[materialize().length];
        for (int i = 0; i < materialize().length; i++) {
            action.accept(materialize()[i]);
            result[i] = materialize()[i];
        }
        return new IntStreamImpl(result);
    }

    @Override
    public IntStream limit(long maxSize) {
        if (maxSize < 0) throw new IllegalArgumentException(Long.toString(maxSize));
        int len = (int) Math.min(materialize().length, maxSize);
        return new IntStreamImpl(Arrays.copyOf(materialize(), len));
    }

    @Override
    public IntStream skip(long n) {
        if (n < 0) throw new IllegalArgumentException(Long.toString(n));
        int skip = (int) Math.min(materialize().length, n);
        return new IntStreamImpl(Arrays.copyOfRange(materialize(), skip, materialize().length));
    }

    @Override
    public void forEach(IntConsumer action) {
        for (int e : materialize()) {
            action.accept(e);
        }
    }

    @Override
    public void forEachOrdered(IntConsumer action) {
        forEach(action);
    }

    @Override
    public int[] toArray() {
        return Arrays.copyOf(materialize(), materialize().length);
    }

    @Override
    public int reduce(int identity, IntBinaryOperator op) {
        int result = identity;
        for (int e : materialize()) {
            result = op.applyAsInt(result, e);
        }
        return result;
    }

    @Override
    public OptionalInt reduce(IntBinaryOperator op) {
        if (materialize().length == 0) return OptionalInt.empty();
        int result = materialize()[0];
        for (int i = 1; i < materialize().length; i++) {
            result = op.applyAsInt(result, materialize()[i]);
        }
        return OptionalInt.of(result);
    }

    @Override
    public <R> R collect(Supplier<R> supplier,
                         ObjIntConsumer<R> accumulator,
                         BiConsumer<R, R> combiner) {
        R result = supplier.get();
        for (int e : materialize()) {
            accumulator.accept(result, e);
        }
        return result;
    }

    @Override
    public int sum() {
        int sum = 0;
        for (int e : materialize()) {
            sum += e;
        }
        return sum;
    }

    @Override
    public OptionalInt min() {
        if (materialize().length == 0) return OptionalInt.empty();
        int min = materialize()[0];
        for (int i = 1; i < materialize().length; i++) {
            if (materialize()[i] < min) min = materialize()[i];
        }
        return OptionalInt.of(min);
    }

    @Override
    public OptionalInt max() {
        if (materialize().length == 0) return OptionalInt.empty();
        int max = materialize()[0];
        for (int i = 1; i < materialize().length; i++) {
            if (materialize()[i] > max) max = materialize()[i];
        }
        return OptionalInt.of(max);
    }

    @Override
    public long count() {
        return materialize().length;
    }

    @Override
    public OptionalDouble average() {
        if (materialize().length == 0) return OptionalDouble.empty();
        long sum = 0;
        for (int e : materialize()) {
            sum += e;
        }
        return OptionalDouble.of((double) sum / materialize().length);
    }

    @Override
    public java.util.IntSummaryStatistics summaryStatistics() {
        throw new UnsupportedOperationException("summaryStatistics");
    }

    @Override
    public boolean anyMatch(IntPredicate predicate) {
        for (int e : materialize()) {
            if (predicate.test(e)) return true;
        }
        return false;
    }

    @Override
    public boolean allMatch(IntPredicate predicate) {
        for (int e : materialize()) {
            if (!predicate.test(e)) return false;
        }
        return true;
    }

    @Override
    public boolean noneMatch(IntPredicate predicate) {
        for (int e : materialize()) {
            if (predicate.test(e)) return false;
        }
        return true;
    }

    @Override
    public OptionalInt findFirst() {
        if (materialize().length == 0) return OptionalInt.empty();
        return OptionalInt.of(materialize()[0]);
    }

    @Override
    public OptionalInt findAny() {
        if (materialize().length == 0) return OptionalInt.empty();
        return OptionalInt.of(materialize()[0]);
    }

    @Override
    public LongStream asLongStream() {
        long[] result = new long[materialize().length];
        for (int i = 0; i < materialize().length; i++) {
            result[i] = materialize()[i];
        }
        return new LongStreamImpl(result);
    }

    @Override
    public DoubleStream asDoubleStream() {
        double[] result = new double[materialize().length];
        for (int i = 0; i < materialize().length; i++) {
            result[i] = materialize()[i];
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public Stream<Integer> boxed() {
        List<Integer> result = new ArrayList<>();
        for (int e : materialize()) {
            result.add(e);
        }
        return new StreamImpl<>(result);
    }

    // BaseStream methods

    @Override
    public java.util.PrimitiveIterator.OfInt iterator() {
        return new java.util.PrimitiveIterator.OfInt() {
            private int index = 0;

            @Override
            public boolean hasNext() {
                return index < materialize().length;
            }

            @Override
            public int nextInt() {
                if (!hasNext()) throw new NoSuchElementException();
                return materialize()[index++];
            }
        };
    }

    @Override
    public Spliterator.OfInt spliterator() {
        return Spliterators.spliterator(materialize(), 0, materialize().length,
                Spliterator.ORDERED | Spliterator.SIZED | Spliterator.SUBSIZED);
    }

    @Override
    public boolean isParallel() {
        return parallel;
    }

    @Override
    public IntStream sequential() {
        this.parallel = false;
        return this;
    }

    @Override
    public IntStream parallel() {
        this.parallel = true;
        return this;
    }

    @Override
    public IntStream unordered() {
        return this;
    }

    @Override
    public IntStream onClose(Runnable closeHandler) {
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
