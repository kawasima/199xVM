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
import java.util.Collection;
import java.util.Collections;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.Set;
import java.util.LinkedHashMap;
import java.util.function.BiConsumer;
import java.util.function.BinaryOperator;
import java.util.function.Function;
import java.util.function.Predicate;
import java.util.function.Supplier;
import java.util.function.ToIntFunction;
import java.util.function.ToLongFunction;
import java.util.function.ToDoubleFunction;

public final class Collectors {
    private Collectors() {}

    // Package-private constant sets used by Collector.of() and CollectorImpl
    static final Set<Collector.Characteristics> CH_ID =
            Collections.unmodifiableSet(EnumSet.of(Collector.Characteristics.IDENTITY_FINISH));
    static final Set<Collector.Characteristics> CH_NOID =
            Collections.emptySet();
    static final Set<Collector.Characteristics> CH_UNORDERED_ID =
            Collections.unmodifiableSet(EnumSet.of(Collector.Characteristics.UNORDERED,
                                                   Collector.Characteristics.IDENTITY_FINISH));
    static final Set<Collector.Characteristics> CH_UNORDERED_NOID =
            Collections.unmodifiableSet(EnumSet.of(Collector.Characteristics.UNORDERED));

    // ---- CollectorImpl (package-private, used by Collector.of) ----

    static class CollectorImpl<T, A, R> implements Collector<T, A, R> {
        private final Supplier<A> supplier;
        private final BiConsumer<A, T> accumulator;
        private final BinaryOperator<A> combiner;
        private final Function<A, R> finisher;
        private final Set<Characteristics> characteristics;

        @SuppressWarnings("unchecked")
        CollectorImpl(Supplier<A> supplier,
                      BiConsumer<A, T> accumulator,
                      BinaryOperator<A> combiner,
                      Set<Characteristics> characteristics) {
            this(supplier, accumulator, combiner, i -> (R) i, characteristics);
        }

        CollectorImpl(Supplier<A> supplier,
                      BiConsumer<A, T> accumulator,
                      BinaryOperator<A> combiner,
                      Function<A, R> finisher,
                      Set<Characteristics> characteristics) {
            this.supplier = supplier;
            this.accumulator = accumulator;
            this.combiner = combiner;
            this.finisher = finisher;
            this.characteristics = characteristics;
        }

        @Override public Supplier<A> supplier() { return supplier; }
        @Override public BiConsumer<A, T> accumulator() { return accumulator; }
        @Override public BinaryOperator<A> combiner() { return combiner; }
        @Override public Function<A, R> finisher() { return finisher; }
        @Override public Set<Characteristics> characteristics() { return characteristics; }
    }

    // ---- Factory methods ----

    public static <T, C extends Collection<T>> Collector<T, ?, C> toCollection(Supplier<C> collectionFactory) {
        return new CollectorImpl<>(collectionFactory,
                                   Collection::add,
                                   (r1, r2) -> { r1.addAll(r2); return r1; },
                                   CH_ID);
    }

    public static <T> Collector<T, ?, List<T>> toList() {
        return new CollectorImpl<>(ArrayList::new,
                                   List::add,
                                   (left, right) -> { left.addAll(right); return left; },
                                   CH_ID);
    }

    public static <T> Collector<T, ?, List<T>> toUnmodifiableList() {
        return new CollectorImpl<>(
                (Supplier<List<T>>) ArrayList::new,
                List::add,
                (left, right) -> { left.addAll(right); return left; },
                list -> Collections.unmodifiableList(list),
                CH_NOID);
    }

    public static <T> Collector<T, ?, Set<T>> toSet() {
        return new CollectorImpl<>(
                (Supplier<Set<T>>) HashSet::new,
                Set::add,
                (left, right) -> { left.addAll(right); return left; },
                CH_UNORDERED_ID);
    }

    public static <T> Collector<T, ?, Set<T>> toUnmodifiableSet() {
        return new CollectorImpl<>(
                (Supplier<Set<T>>) HashSet::new,
                Set::add,
                (left, right) -> { left.addAll(right); return left; },
                set -> Collections.unmodifiableSet(set),
                CH_UNORDERED_NOID);
    }

    public static Collector<CharSequence, ?, String> joining() {
        return new CollectorImpl<>(
                StringBuilder::new,
                StringBuilder::append,
                (sb1, sb2) -> { sb1.append(sb2); return sb1; },
                StringBuilder::toString,
                CH_NOID);
    }

    public static Collector<CharSequence, ?, String> joining(CharSequence delimiter) {
        return joining(delimiter, "", "");
    }

    public static Collector<CharSequence, ?, String> joining(CharSequence delimiter,
                                                             CharSequence prefix,
                                                             CharSequence suffix) {
        String d = delimiter.toString();
        String p = prefix.toString();
        String s = suffix.toString();
        return new CollectorImpl<>(
                () -> new StringBuilder[] { new StringBuilder(), null },
                (arr, cs) -> {
                    if (arr[1] != null) arr[0].append(d);
                    else arr[1] = arr[0];          // mark first element seen
                    arr[0].append(cs);
                },
                (a, b) -> {
                    if (b[1] != null) {             // b has elements
                        if (a[1] != null) a[0].append(d);
                        else a[1] = a[0];
                        a[0].append(b[0]);
                    }
                    return a;
                },
                arr -> p + arr[0].toString() + s,
                CH_NOID);
    }

    public static <T, U, A, R> Collector<T, ?, R> mapping(
            Function<? super T, ? extends U> mapper,
            Collector<? super U, A, R> downstream) {
        BiConsumer<A, ? super U> downstreamAccumulator = downstream.accumulator();
        return new CollectorImpl<>(downstream.supplier(),
                                   (r, t) -> downstreamAccumulator.accept(r, mapper.apply(t)),
                                   downstream.combiner(),
                                   downstream.finisher(),
                                   downstream.characteristics());
    }

    public static <T, A, R> Collector<T, ?, R> filtering(
            Predicate<? super T> predicate,
            Collector<? super T, A, R> downstream) {
        BiConsumer<A, ? super T> downstreamAccumulator = downstream.accumulator();
        return new CollectorImpl<>(downstream.supplier(),
                                   (r, t) -> {
                                       if (predicate.test(t))
                                           downstreamAccumulator.accept(r, t);
                                   },
                                   downstream.combiner(),
                                   downstream.finisher(),
                                   downstream.characteristics());
    }

    public static <T, U, A, R> Collector<T, ?, R> flatMapping(
            Function<? super T, ? extends Stream<? extends U>> mapper,
            Collector<? super U, A, R> downstream) {
        BiConsumer<A, ? super U> downstreamAccumulator = downstream.accumulator();
        return new CollectorImpl<>(downstream.supplier(),
                                   (r, t) -> {
                                       Stream<? extends U> s = mapper.apply(t);
                                       if (s != null) {
                                           s.forEach(u -> downstreamAccumulator.accept(r, u));
                                       }
                                   },
                                   downstream.combiner(),
                                   downstream.finisher(),
                                   downstream.characteristics());
    }

    public static <T> Collector<T, ?, Long> counting() {
        return new CollectorImpl<>(
                () -> new long[]{0},
                (a, t) -> a[0]++,
                (a, b) -> { a[0] += b[0]; return a; },
                a -> a[0],
                CH_NOID);
    }

    public static <T> Collector<T, ?, Optional<T>> reducing(BinaryOperator<T> op) {
        @SuppressWarnings("unchecked")
        class OptionalBox {
            T value;
            boolean present;
        }
        return new CollectorImpl<>(
                OptionalBox::new,
                (box, t) -> {
                    if (box.present) {
                        box.value = op.apply(box.value, t);
                    } else {
                        box.value = t;
                        box.present = true;
                    }
                },
                (a, b) -> {
                    if (b.present) {
                        if (a.present) {
                            a.value = op.apply(a.value, b.value);
                        } else {
                            a.value = b.value;
                            a.present = true;
                        }
                    }
                    return a;
                },
                box -> box.present ? Optional.of(box.value) : Optional.empty(),
                CH_NOID);
    }

    public static <T> Collector<T, ?, T> reducing(T identity, BinaryOperator<T> op) {
        return new CollectorImpl<>(
                () -> {
                    @SuppressWarnings("unchecked")
                    T[] box = (T[]) new Object[]{identity};
                    return box;
                },
                (a, t) -> a[0] = op.apply(a[0], t),
                (a, b) -> { a[0] = op.apply(a[0], b[0]); return a; },
                a -> a[0],
                CH_NOID);
    }

    public static <T, U> Collector<T, ?, U> reducing(U identity,
                                                     Function<? super T, ? extends U> mapper,
                                                     BinaryOperator<U> op) {
        return new CollectorImpl<>(
                () -> {
                    @SuppressWarnings("unchecked")
                    U[] box = (U[]) new Object[]{identity};
                    return box;
                },
                (a, t) -> a[0] = op.apply(a[0], mapper.apply(t)),
                (a, b) -> { a[0] = op.apply(a[0], b[0]); return a; },
                a -> a[0],
                CH_NOID);
    }

    public static <T, K> Collector<T, ?, Map<K, List<T>>> groupingBy(
            Function<? super T, ? extends K> classifier) {
        return groupingBy(classifier, toList());
    }

    public static <T, K, A, D> Collector<T, ?, Map<K, D>> groupingBy(
            Function<? super T, ? extends K> classifier,
            Collector<? super T, A, D> downstream) {
        return groupingBy(classifier, HashMap::new, downstream);
    }

    public static <T, K, D, A, M extends Map<K, D>> Collector<T, ?, M> groupingBy(
            Function<? super T, ? extends K> classifier,
            Supplier<M> mapFactory,
            Collector<? super T, A, D> downstream) {
        Supplier<A> downstreamSupplier = downstream.supplier();
        BiConsumer<A, ? super T> downstreamAccumulator = downstream.accumulator();
        Function<A, D> downstreamFinisher = downstream.finisher();

        @SuppressWarnings("unchecked")
        BiConsumer<Map<K, A>, T> accumulator = (m, t) -> {
            K key = Objects.requireNonNull(classifier.apply(t), "element cannot be mapped to a null key");
            A container = m.computeIfAbsent(key, k -> downstreamSupplier.get());
            downstreamAccumulator.accept(container, t);
        };

        BinaryOperator<Map<K, A>> merger = mapMerger(downstream.combiner());

        @SuppressWarnings("unchecked")
        Supplier<Map<K, A>> mangledFactory = (Supplier<Map<K, A>>) (Supplier) mapFactory;

        if (downstream.characteristics().contains(Collector.Characteristics.IDENTITY_FINISH)) {
            @SuppressWarnings("unchecked")
            Function<Map<K, A>, M> castingFinisher = i -> (M) i;
            return new CollectorImpl<>(mangledFactory, accumulator, merger, castingFinisher, CH_NOID);
        } else {
            Function<Map<K, A>, M> finisher = intermediate -> {
                @SuppressWarnings("unchecked")
                M result = (M) mapFactory.get();
                for (Map.Entry<K, A> entry : intermediate.entrySet()) {
                    result.put(entry.getKey(), downstreamFinisher.apply(entry.getValue()));
                }
                return result;
            };
            return new CollectorImpl<>(mangledFactory, accumulator, merger, finisher, CH_NOID);
        }
    }

    public static <T> Collector<T, ?, Map<Boolean, List<T>>> partitioningBy(
            Predicate<? super T> predicate) {
        return partitioningBy(predicate, toList());
    }

    public static <T, D, A> Collector<T, ?, Map<Boolean, D>> partitioningBy(
            Predicate<? super T> predicate,
            Collector<? super T, A, D> downstream) {
        Supplier<A> downstreamSupplier = downstream.supplier();
        BiConsumer<A, ? super T> downstreamAccumulator = downstream.accumulator();
        Function<A, D> downstreamFinisher = downstream.finisher();

        // Use Object[] to hold the two partitions: [0]=false, [1]=true
        Supplier<Object[]> supplier = () -> new Object[]{ downstreamSupplier.get(), downstreamSupplier.get() };
        @SuppressWarnings("unchecked")
        BiConsumer<Object[], T> accumulator = (pair, t) -> {
            downstreamAccumulator.accept((A) pair[predicate.test(t) ? 1 : 0], t);
        };
        BinaryOperator<Object[]> combiner = (a, b) -> {
            @SuppressWarnings("unchecked")
            A left0 = downstream.combiner().apply((A) a[0], (A) b[0]);
            @SuppressWarnings("unchecked")
            A left1 = downstream.combiner().apply((A) a[1], (A) b[1]);
            a[0] = left0;
            a[1] = left1;
            return a;
        };
        @SuppressWarnings("unchecked")
        Function<Object[], Map<Boolean, D>> finisher = pair -> {
            Map<Boolean, D> result = new HashMap<>();
            result.put(false, downstreamFinisher.apply((A) pair[0]));
            result.put(true, downstreamFinisher.apply((A) pair[1]));
            return result;
        };
        return new CollectorImpl<>(supplier, accumulator, combiner, finisher, CH_NOID);
    }

    public static <T, K, U> Collector<T, ?, Map<K, U>> toMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper) {
        return toMap(keyMapper, valueMapper, throwingMerger(), HashMap::new);
    }

    public static <T, K, U> Collector<T, ?, Map<K, U>> toMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper,
            BinaryOperator<U> mergeFunction) {
        return toMap(keyMapper, valueMapper, mergeFunction, HashMap::new);
    }

    public static <T, K, U, M extends Map<K, U>> Collector<T, ?, M> toMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper,
            BinaryOperator<U> mergeFunction,
            Supplier<M> mapFactory) {
        BiConsumer<M, T> accumulator = (map, element) -> {
            K key = keyMapper.apply(element);
            U value = valueMapper.apply(element);
            U existing = map.get(key);
            U merged = (existing == null) ? value : mergeFunction.apply(existing, value);
            map.put(key, merged);
        };
        return new CollectorImpl<>(mapFactory, accumulator,
                                   mapMerger2(mergeFunction), CH_ID);
    }

    public static <T, K, U> Collector<T, ?, Map<K, U>> toUnmodifiableMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper) {
        return toUnmodifiableMap(keyMapper, valueMapper, throwingMerger());
    }

    public static <T, K, U> Collector<T, ?, Map<K, U>> toUnmodifiableMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper,
            BinaryOperator<U> mergeFunction) {
        return new CollectorImpl<>(
                (Supplier<Map<K, U>>) HashMap::new,
                (map, element) -> {
                    K key = keyMapper.apply(element);
                    U value = valueMapper.apply(element);
                    U existing = map.get(key);
                    U merged = (existing == null) ? value : mergeFunction.apply(existing, value);
                    map.put(key, merged);
                },
                mapMerger2(mergeFunction),
                map -> Collections.unmodifiableMap(map),
                CH_NOID);
    }

    public static <T> Collector<T, ?, Integer> summingInt(ToIntFunction<? super T> mapper) {
        return new CollectorImpl<>(
                () -> new int[]{0},
                (a, t) -> a[0] += mapper.applyAsInt(t),
                (a, b) -> { a[0] += b[0]; return a; },
                a -> a[0],
                CH_NOID);
    }

    public static <T> Collector<T, ?, Long> summingLong(ToLongFunction<? super T> mapper) {
        return new CollectorImpl<>(
                () -> new long[]{0},
                (a, t) -> a[0] += mapper.applyAsLong(t),
                (a, b) -> { a[0] += b[0]; return a; },
                a -> a[0],
                CH_NOID);
    }

    public static <T> Collector<T, ?, Double> summingDouble(ToDoubleFunction<? super T> mapper) {
        return new CollectorImpl<>(
                () -> new double[]{0},
                (a, t) -> a[0] += mapper.applyAsDouble(t),
                (a, b) -> { a[0] += b[0]; return a; },
                a -> a[0],
                CH_NOID);
    }

    // ---- helpers ----

    private static <T> BinaryOperator<T> throwingMerger() {
        return (u, v) -> { throw new IllegalStateException("Duplicate key (attempted merging values " + u + " and " + v + ")"); };
    }

    private static <K, A> BinaryOperator<Map<K, A>> mapMerger(BinaryOperator<A> mergeFunction) {
        return (m1, m2) -> {
            for (Map.Entry<K, A> e : m2.entrySet()) {
                A val = m1.get(e.getKey());
                m1.put(e.getKey(), val == null ? e.getValue() : mergeFunction.apply(val, e.getValue()));
            }
            return m1;
        };
    }

    private static <K, U, M extends Map<K, U>> BinaryOperator<M> mapMerger2(BinaryOperator<U> mergeFunction) {
        return (m1, m2) -> {
            for (Map.Entry<K, U> e : m2.entrySet()) {
                U val = m1.get(e.getKey());
                m1.put(e.getKey(), val == null ? e.getValue() : mergeFunction.apply(val, e.getValue()));
            }
            return m1;
        };
    }
}
