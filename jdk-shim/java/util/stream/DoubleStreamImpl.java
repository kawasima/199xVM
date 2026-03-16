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
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.function.BiConsumer;
import java.util.function.DoubleBinaryOperator;
import java.util.function.DoubleConsumer;
import java.util.function.DoubleFunction;
import java.util.function.DoublePredicate;
import java.util.function.DoubleToIntFunction;
import java.util.function.DoubleToLongFunction;
import java.util.function.DoubleUnaryOperator;
import java.util.function.ObjDoubleConsumer;
import java.util.function.Supplier;

public class DoubleStreamImpl implements DoubleStream {
    private final double[] elements;
    private boolean parallel;
    private List<Runnable> closeHandlers;

    public DoubleStreamImpl(double[] elements) {
        this.elements = elements;
    }

    @Override
    public DoubleStream filter(DoublePredicate predicate) {
        int count = 0;
        for (double e : elements) {
            if (predicate.test(e)) count++;
        }
        double[] result = new double[count];
        int idx = 0;
        for (double e : elements) {
            if (predicate.test(e)) result[idx++] = e;
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public DoubleStream map(DoubleUnaryOperator mapper) {
        double[] result = new double[elements.length];
        for (int i = 0; i < elements.length; i++) {
            result[i] = mapper.applyAsDouble(elements[i]);
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public <U> Stream<U> mapToObj(DoubleFunction<? extends U> mapper) {
        List<U> result = new ArrayList<>();
        for (double e : elements) {
            result.add(mapper.apply(e));
        }
        return new StreamImpl<>(result);
    }

    @Override
    public IntStream mapToInt(DoubleToIntFunction mapper) {
        int[] result = new int[elements.length];
        for (int i = 0; i < elements.length; i++) {
            result[i] = mapper.applyAsInt(elements[i]);
        }
        return new IntStreamImpl(result);
    }

    @Override
    public LongStream mapToLong(DoubleToLongFunction mapper) {
        long[] result = new long[elements.length];
        for (int i = 0; i < elements.length; i++) {
            result[i] = mapper.applyAsLong(elements[i]);
        }
        return new LongStreamImpl(result);
    }

    @Override
    public DoubleStream flatMap(DoubleFunction<? extends DoubleStream> mapper) {
        double[] buf = new double[elements.length * 4];
        int size = 0;
        for (double e : elements) {
            DoubleStream s = mapper.apply(e);
            if (s != null) {
                try {
                    double[] arr = s.toArray();
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
        return new DoubleStreamImpl(Arrays.copyOf(buf, size));
    }

    @Override
    public DoubleStream distinct() {
        java.util.LinkedHashSet<Double> seen = new java.util.LinkedHashSet<>();
        for (double e : elements) {
            seen.add(e);
        }
        double[] result = new double[seen.size()];
        int idx = 0;
        for (double v : seen) {
            result[idx++] = v;
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public DoubleStream sorted() {
        double[] result = Arrays.copyOf(elements, elements.length);
        Arrays.sort(result);
        return new DoubleStreamImpl(result);
    }

    @Override
    public DoubleStream peek(DoubleConsumer action) {
        double[] result = new double[elements.length];
        for (int i = 0; i < elements.length; i++) {
            action.accept(elements[i]);
            result[i] = elements[i];
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public DoubleStream limit(long maxSize) {
        if (maxSize < 0) throw new IllegalArgumentException(Long.toString(maxSize));
        int len = (int) Math.min(elements.length, maxSize);
        return new DoubleStreamImpl(Arrays.copyOf(elements, len));
    }

    @Override
    public DoubleStream skip(long n) {
        if (n < 0) throw new IllegalArgumentException(Long.toString(n));
        int skip = (int) Math.min(elements.length, n);
        return new DoubleStreamImpl(Arrays.copyOfRange(elements, skip, elements.length));
    }

    @Override
    public void forEach(DoubleConsumer action) {
        for (double e : elements) {
            action.accept(e);
        }
    }

    @Override
    public void forEachOrdered(DoubleConsumer action) {
        forEach(action);
    }

    @Override
    public double[] toArray() {
        return Arrays.copyOf(elements, elements.length);
    }

    @Override
    public double reduce(double identity, DoubleBinaryOperator op) {
        double result = identity;
        for (double e : elements) {
            result = op.applyAsDouble(result, e);
        }
        return result;
    }

    @Override
    public OptionalDouble reduce(DoubleBinaryOperator op) {
        if (elements.length == 0) return OptionalDouble.empty();
        double result = elements[0];
        for (int i = 1; i < elements.length; i++) {
            result = op.applyAsDouble(result, elements[i]);
        }
        return OptionalDouble.of(result);
    }

    @Override
    public <R> R collect(Supplier<R> supplier,
                         ObjDoubleConsumer<R> accumulator,
                         BiConsumer<R, R> combiner) {
        R result = supplier.get();
        for (double e : elements) {
            accumulator.accept(result, e);
        }
        return result;
    }

    @Override
    public double sum() {
        double sum = 0;
        for (double e : elements) {
            sum += e;
        }
        return sum;
    }

    @Override
    public OptionalDouble min() {
        if (elements.length == 0) return OptionalDouble.empty();
        double min = elements[0];
        for (int i = 1; i < elements.length; i++) {
            if (Double.compare(elements[i], min) < 0) min = elements[i];
        }
        return OptionalDouble.of(min);
    }

    @Override
    public OptionalDouble max() {
        if (elements.length == 0) return OptionalDouble.empty();
        double max = elements[0];
        for (int i = 1; i < elements.length; i++) {
            if (Double.compare(elements[i], max) > 0) max = elements[i];
        }
        return OptionalDouble.of(max);
    }

    @Override
    public long count() {
        return elements.length;
    }

    @Override
    public OptionalDouble average() {
        if (elements.length == 0) return OptionalDouble.empty();
        double sum = 0;
        for (double e : elements) {
            sum += e;
        }
        return OptionalDouble.of(sum / elements.length);
    }

    @Override
    public java.util.DoubleSummaryStatistics summaryStatistics() {
        throw new UnsupportedOperationException("summaryStatistics");
    }

    @Override
    public boolean anyMatch(DoublePredicate predicate) {
        for (double e : elements) {
            if (predicate.test(e)) return true;
        }
        return false;
    }

    @Override
    public boolean allMatch(DoublePredicate predicate) {
        for (double e : elements) {
            if (!predicate.test(e)) return false;
        }
        return true;
    }

    @Override
    public boolean noneMatch(DoublePredicate predicate) {
        for (double e : elements) {
            if (predicate.test(e)) return false;
        }
        return true;
    }

    @Override
    public OptionalDouble findFirst() {
        if (elements.length == 0) return OptionalDouble.empty();
        return OptionalDouble.of(elements[0]);
    }

    @Override
    public OptionalDouble findAny() {
        if (elements.length == 0) return OptionalDouble.empty();
        return OptionalDouble.of(elements[0]);
    }

    @Override
    public Stream<Double> boxed() {
        List<Double> result = new ArrayList<>();
        for (double e : elements) {
            result.add(e);
        }
        return new StreamImpl<>(result);
    }

    // BaseStream methods

    @Override
    public java.util.PrimitiveIterator.OfDouble iterator() {
        return new java.util.PrimitiveIterator.OfDouble() {
            private int index = 0;

            @Override
            public boolean hasNext() {
                return index < elements.length;
            }

            @Override
            public double nextDouble() {
                if (!hasNext()) throw new NoSuchElementException();
                return elements[index++];
            }
        };
    }

    @Override
    public Spliterator.OfDouble spliterator() {
        return Spliterators.spliterator(elements, 0, elements.length,
                Spliterator.ORDERED | Spliterator.SIZED | Spliterator.SUBSIZED);
    }

    @Override
    public boolean isParallel() {
        return parallel;
    }

    @Override
    public DoubleStream sequential() {
        this.parallel = false;
        return this;
    }

    @Override
    public DoubleStream parallel() {
        this.parallel = true;
        return this;
    }

    @Override
    public DoubleStream unordered() {
        return this;
    }

    @Override
    public DoubleStream onClose(Runnable closeHandler) {
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
