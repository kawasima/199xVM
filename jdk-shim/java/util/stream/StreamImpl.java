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
import java.util.Collections;
import java.util.Comparator;
import java.util.Iterator;
import java.util.List;
import java.util.Optional;
import java.util.Spliterator;
import java.util.function.BinaryOperator;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.IntFunction;
import java.util.function.Predicate;
import java.util.function.ToIntFunction;
import java.util.function.ToLongFunction;
import java.util.function.ToDoubleFunction;

public class StreamImpl<T> implements Stream<T> {
    private final List<T> elements;
    private boolean parallel;
    private List<Runnable> closeHandlers;

    public StreamImpl(List<T> elements) {
        this.elements = elements;
    }

    @Override
    @SuppressWarnings("unchecked")
    public <R> Stream<R> map(Function<? super T, ? extends R> mapper) {
        List<R> result = new ArrayList<>();
        for (T e : elements) {
            result.add(mapper.apply(e));
        }
        return new StreamImpl<>(result);
    }

    @Override
    @SuppressWarnings("unchecked")
    public <R> Stream<R> flatMap(Function<? super T, ? extends Stream<? extends R>> mapper) {
        List<R> result = new ArrayList<>();
        for (T e : elements) {
            Stream<? extends R> s = mapper.apply(e);
            if (s != null) {
                try {
                    s.forEach(r -> result.add(r));
                } finally {
                    s.close();
                }
            }
        }
        return new StreamImpl<>(result);
    }

    @Override
    public IntStream mapToInt(ToIntFunction<? super T> mapper) {
        int[] result = new int[elements.size()];
        for (int i = 0; i < elements.size(); i++) {
            result[i] = mapper.applyAsInt(elements.get(i));
        }
        return new IntStreamImpl(result);
    }

    @Override
    public LongStream mapToLong(ToLongFunction<? super T> mapper) {
        long[] result = new long[elements.size()];
        for (int i = 0; i < elements.size(); i++) {
            result[i] = mapper.applyAsLong(elements.get(i));
        }
        return new LongStreamImpl(result);
    }

    @Override
    public DoubleStream mapToDouble(ToDoubleFunction<? super T> mapper) {
        double[] result = new double[elements.size()];
        for (int i = 0; i < elements.size(); i++) {
            result[i] = mapper.applyAsDouble(elements.get(i));
        }
        return new DoubleStreamImpl(result);
    }

    @Override
    public IntStream flatMapToInt(Function<? super T, ? extends IntStream> mapper) {
        int[] buf = new int[16];
        int size = 0;
        for (T e : elements) {
            IntStream s = mapper.apply(e);
            if (s != null) {
                try {
                    int[] arr = s.toArray();
                    while (size + arr.length > buf.length) {
                        buf = java.util.Arrays.copyOf(buf, buf.length * 2);
                    }
                    System.arraycopy(arr, 0, buf, size, arr.length);
                    size += arr.length;
                } finally {
                    s.close();
                }
            }
        }
        return new IntStreamImpl(java.util.Arrays.copyOf(buf, size));
    }

    @Override
    public LongStream flatMapToLong(Function<? super T, ? extends LongStream> mapper) {
        long[] buf = new long[16];
        int size = 0;
        for (T e : elements) {
            LongStream s = mapper.apply(e);
            if (s != null) {
                try {
                    long[] arr = s.toArray();
                    while (size + arr.length > buf.length) {
                        buf = java.util.Arrays.copyOf(buf, buf.length * 2);
                    }
                    System.arraycopy(arr, 0, buf, size, arr.length);
                    size += arr.length;
                } finally {
                    s.close();
                }
            }
        }
        return new LongStreamImpl(java.util.Arrays.copyOf(buf, size));
    }

    @Override
    public DoubleStream flatMapToDouble(Function<? super T, ? extends DoubleStream> mapper) {
        double[] buf = new double[16];
        int size = 0;
        for (T e : elements) {
            DoubleStream s = mapper.apply(e);
            if (s != null) {
                try {
                    double[] arr = s.toArray();
                    while (size + arr.length > buf.length) {
                        buf = java.util.Arrays.copyOf(buf, buf.length * 2);
                    }
                    System.arraycopy(arr, 0, buf, size, arr.length);
                    size += arr.length;
                } finally {
                    s.close();
                }
            }
        }
        return new DoubleStreamImpl(java.util.Arrays.copyOf(buf, size));
    }

    @Override
    public Stream<T> filter(Predicate<? super T> predicate) {
        List<T> result = new ArrayList<>();
        for (T e : elements) {
            if (predicate.test(e)) result.add(e);
        }
        return new StreamImpl<>(result);
    }

    @Override
    public Stream<T> distinct() {
        java.util.LinkedHashSet<T> seen = new java.util.LinkedHashSet<>();
        for (T e : elements) {
            seen.add(e);
        }
        return new StreamImpl<>(new ArrayList<>(seen));
    }

    @Override
    @SuppressWarnings("unchecked")
    public Stream<T> sorted() {
        List<T> result = new ArrayList<>(elements);
        Collections.sort((List<Comparable>) result);
        return new StreamImpl<>(result);
    }

    @Override
    public Stream<T> sorted(Comparator<? super T> comparator) {
        List<T> result = new ArrayList<>(elements);
        result.sort(comparator);
        return new StreamImpl<>(result);
    }

    @Override
    public Stream<T> peek(Consumer<? super T> action) {
        List<T> result = new ArrayList<>();
        for (T e : elements) {
            action.accept(e);
            result.add(e);
        }
        return new StreamImpl<>(result);
    }

    @Override
    public Stream<T> limit(long maxSize) {
        if (maxSize < 0) throw new IllegalArgumentException(Long.toString(maxSize));
        List<T> result = new ArrayList<>();
        long count = 0;
        for (T e : elements) {
            if (count >= maxSize) break;
            result.add(e);
            count++;
        }
        return new StreamImpl<>(result);
    }

    @Override
    public Stream<T> skip(long n) {
        if (n < 0) throw new IllegalArgumentException(Long.toString(n));
        List<T> result = new ArrayList<>();
        long count = 0;
        for (T e : elements) {
            if (count >= n) result.add(e);
            count++;
        }
        return new StreamImpl<>(result);
    }

    @Override
    public void forEach(Consumer<? super T> action) {
        for (T e : elements) {
            action.accept(e);
        }
    }

    @Override
    public void forEachOrdered(Consumer<? super T> action) {
        forEach(action);
    }

    @Override
    public Object[] toArray() {
        return elements.toArray();
    }

    @Override
    @SuppressWarnings("unchecked")
    public <A> A[] toArray(IntFunction<A[]> generator) {
        A[] arr = generator.apply(elements.size());
        for (int i = 0; i < elements.size(); i++) {
            arr[i] = (A) elements.get(i);
        }
        return arr;
    }

    @Override
    public T reduce(T identity, BinaryOperator<T> accumulator) {
        T result = identity;
        for (T e : elements) {
            result = accumulator.apply(result, e);
        }
        return result;
    }

    @Override
    public Optional<T> reduce(BinaryOperator<T> accumulator) {
        if (elements.isEmpty()) return Optional.empty();
        T result = elements.get(0);
        for (int i = 1; i < elements.size(); i++) {
            result = accumulator.apply(result, elements.get(i));
        }
        return Optional.of(result);
    }

    @Override
    public <U> U reduce(U identity,
                        java.util.function.BiFunction<U, ? super T, U> accumulator,
                        BinaryOperator<U> combiner) {
        U result = identity;
        for (T e : elements) {
            result = accumulator.apply(result, e);
        }
        return result;
    }

    @Override
    public <R> R collect(java.util.function.Supplier<R> supplier,
                         java.util.function.BiConsumer<R, ? super T> accumulator,
                         java.util.function.BiConsumer<R, R> combiner) {
        R result = supplier.get();
        for (T e : elements) {
            accumulator.accept(result, e);
        }
        return result;
    }

    @Override
    public <R, A> R collect(Collector<? super T, A, R> collector) {
        A container = collector.supplier().get();
        java.util.function.BiConsumer<A, ? super T> acc = collector.accumulator();
        for (T e : elements) {
            acc.accept(container, e);
        }
        return collector.finisher().apply(container);
    }

    @Override
    public Optional<T> min(Comparator<? super T> comparator) {
        if (elements.isEmpty()) return Optional.empty();
        T min = elements.get(0);
        for (int i = 1; i < elements.size(); i++) {
            if (comparator.compare(elements.get(i), min) < 0) {
                min = elements.get(i);
            }
        }
        return Optional.of(min);
    }

    @Override
    public Optional<T> max(Comparator<? super T> comparator) {
        if (elements.isEmpty()) return Optional.empty();
        T max = elements.get(0);
        for (int i = 1; i < elements.size(); i++) {
            if (comparator.compare(elements.get(i), max) > 0) {
                max = elements.get(i);
            }
        }
        return Optional.of(max);
    }

    @Override
    public long count() {
        return elements.size();
    }

    @Override
    public boolean anyMatch(Predicate<? super T> predicate) {
        for (T e : elements) {
            if (predicate.test(e)) return true;
        }
        return false;
    }

    @Override
    public boolean allMatch(Predicate<? super T> predicate) {
        for (T e : elements) {
            if (!predicate.test(e)) return false;
        }
        return true;
    }

    @Override
    public boolean noneMatch(Predicate<? super T> predicate) {
        for (T e : elements) {
            if (predicate.test(e)) return false;
        }
        return true;
    }

    @Override
    public Optional<T> findFirst() {
        if (elements.isEmpty()) {
            return Optional.empty();
        }
        return Optional.of(elements.get(0));
    }

    @Override
    public Optional<T> findAny() {
        if (elements.isEmpty()) {
            return Optional.empty();
        }
        return Optional.of(elements.get(0));
    }

    // BaseStream methods

    @Override
    public Iterator<T> iterator() {
        return elements.iterator();
    }

    @Override
    public Spliterator<T> spliterator() {
        return elements.spliterator();
    }

    @Override
    public boolean isParallel() {
        return parallel;
    }

    @Override
    public Stream<T> sequential() {
        this.parallel = false;
        return this;
    }

    @Override
    public Stream<T> parallel() {
        this.parallel = true;
        return this;
    }

    @Override
    @SuppressWarnings("unchecked")
    public Stream<T> unordered() {
        return this;
    }

    @Override
    public Stream<T> onClose(Runnable closeHandler) {
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
