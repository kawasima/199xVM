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
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.BinaryOperator;
import java.util.function.Function;
import java.util.function.Supplier;

public final class Collectors {
    private Collectors() {}

    public static Collector<CharSequence, StringBuilder, String> joining(
            CharSequence delimiter, CharSequence prefix, CharSequence suffix) {
        return new JoiningCollector(delimiter.toString(), prefix.toString(), suffix.toString());
    }

    public static Collector<CharSequence, StringBuilder, String> joining(CharSequence delimiter) {
        return new JoiningCollector(delimiter.toString(), "", "");
    }

    public static Collector<CharSequence, StringBuilder, String> joining() {
        return new JoiningCollector("", "", "");
    }

    public static <T> Collector<T, List<T>, List<T>> toList() {
        return new ToListCollector<>();
    }

    public static <T> Collector<T, Set<T>, Set<T>> toSet() {
        return new ToSetCollector<>();
    }

    public static <T, U, A, R> Collector<T, A, R> mapping(
            Function<? super T, ? extends U> mapper,
            Collector<? super U, A, R> downstream) {
        return new MappingCollector<>(mapper, downstream);
    }

    public static <T, K> Collector<T, Map<K, List<T>>, Map<K, List<T>>> groupingBy(
            Function<? super T, ? extends K> classifier) {
        return groupingBy(classifier, HashMap::new, toList());
    }

    public static <T, K, A, D> Collector<T, Map<K, A>, Map<K, D>> groupingBy(
            Function<? super T, ? extends K> classifier,
            Supplier<Map<K, A>> mapFactory,
            Collector<? super T, A, D> downstream) {
        return new GroupingByCollector<>(classifier, mapFactory, downstream);
    }

    public static <T, K, U> Collector<T, Map<K, U>, Map<K, U>> toMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper) {
        return new ToMapCollector<>(keyMapper, valueMapper, HashMap::new);
    }

    public static <T, K, U> Collector<T, Map<K, U>, Map<K, U>> toMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper,
            BinaryOperator<U> mergeFunction,
            Supplier<Map<K, U>> mapFactory) {
        return new ToMapCollector<>(keyMapper, valueMapper, mapFactory);
    }

    public static <T, K, U> Collector<T, Map<K, U>, Map<K, U>> toUnmodifiableMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper) {
        return new ToUnmodifiableMapCollector<>(keyMapper, valueMapper);
    }

    public static <T, K, U> Collector<T, Map<K, U>, Map<K, U>> toUnmodifiableMap(
            Function<? super T, ? extends K> keyMapper,
            Function<? super T, ? extends U> valueMapper,
            BinaryOperator<U> mergeFunction) {
        return new ToUnmodifiableMapCollector<>(keyMapper, valueMapper);
    }

    public static <T> Collector<T, List<T>, List<T>> toUnmodifiableList() {
        return new ToUnmodifiableListCollector<>();
    }

    private static class JoiningCollector implements Collector<CharSequence, StringBuilder, String> {
        private final String delimiter;
        private final String prefix;
        private final String suffix;

        JoiningCollector(String delimiter, String prefix, String suffix) {
            this.delimiter = delimiter;
            this.prefix = prefix;
            this.suffix = suffix;
        }

        @Override
        public StringBuilder supplier() {
            return new StringBuilder();
        }

        @Override
        public void accumulator(StringBuilder sb, CharSequence element) {
            if (sb.length() > 0) sb.append(delimiter);
            sb.append(element);
        }

        @Override
        public String finisher(StringBuilder sb) {
            return prefix + sb.toString() + suffix;
        }
    }

    private static class ToListCollector<T> implements Collector<T, List<T>, List<T>> {
        @Override
        public List<T> supplier() {
            return new ArrayList<>();
        }

        @Override
        public void accumulator(List<T> list, T element) {
            list.add(element);
        }

        @Override
        public List<T> finisher(List<T> list) {
            return list;
        }
    }

    private static class ToSetCollector<T> implements Collector<T, Set<T>, Set<T>> {
        @Override
        public Set<T> supplier() {
            return new HashSet<>();
        }

        @Override
        public void accumulator(Set<T> set, T element) {
            set.add(element);
        }

        @Override
        public Set<T> finisher(Set<T> set) {
            return set;
        }
    }

    private static class MappingCollector<T, U, A, R> implements Collector<T, A, R> {
        private final Function<? super T, ? extends U> mapper;
        private final Collector<? super U, A, R> downstream;

        MappingCollector(Function<? super T, ? extends U> mapper, Collector<? super U, A, R> downstream) {
            this.mapper = mapper;
            this.downstream = downstream;
        }

        @Override
        public A supplier() {
            return downstream.supplier();
        }

        @Override
        public void accumulator(A container, T element) {
            downstream.accumulator(container, mapper.apply(element));
        }

        @Override
        public R finisher(A container) {
            return downstream.finisher(container);
        }
    }

    private static class GroupingByCollector<T, K, A, D> implements Collector<T, Map<K, A>, Map<K, D>> {
        private final Function<? super T, ? extends K> classifier;
        private final Supplier<Map<K, A>> mapFactory;
        private final Collector<? super T, A, D> downstream;

        GroupingByCollector(Function<? super T, ? extends K> classifier,
                            Supplier<Map<K, A>> mapFactory,
                            Collector<? super T, A, D> downstream) {
            this.classifier = classifier;
            this.mapFactory = mapFactory;
            this.downstream = downstream;
        }

        @Override
        public Map<K, A> supplier() {
            return mapFactory.get();
        }

        @Override
        public void accumulator(Map<K, A> map, T element) {
            K key = classifier.apply(element);
            A container = map.get(key);
            if (container == null) {
                container = downstream.supplier();
                map.put(key, container);
            }
            downstream.accumulator(container, element);
        }

        @Override
        @SuppressWarnings("unchecked")
        public Map<K, D> finisher(Map<K, A> map) {
            Map<K, D> result = (Map<K, D>) mapFactory.get();
            for (Map.Entry<K, A> entry : map.entrySet()) {
                result.put(entry.getKey(), downstream.finisher(entry.getValue()));
            }
            return result;
        }
    }

    private static class ToMapCollector<T, K, U> implements Collector<T, Map<K, U>, Map<K, U>> {
        private final Function<? super T, ? extends K> keyMapper;
        private final Function<? super T, ? extends U> valueMapper;
        private final Supplier<Map<K, U>> mapFactory;

        ToMapCollector(Function<? super T, ? extends K> keyMapper,
                       Function<? super T, ? extends U> valueMapper,
                       Supplier<Map<K, U>> mapFactory) {
            this.keyMapper = keyMapper;
            this.valueMapper = valueMapper;
            this.mapFactory = mapFactory;
        }

        @Override
        public Map<K, U> supplier() { return mapFactory.get(); }

        @Override
        public void accumulator(Map<K, U> map, T element) {
            map.put(keyMapper.apply(element), valueMapper.apply(element));
        }

        @Override
        public Map<K, U> finisher(Map<K, U> map) { return map; }
    }

    private static class ToUnmodifiableMapCollector<T, K, U> implements Collector<T, Map<K, U>, Map<K, U>> {
        private final Function<? super T, ? extends K> keyMapper;
        private final Function<? super T, ? extends U> valueMapper;

        ToUnmodifiableMapCollector(Function<? super T, ? extends K> keyMapper,
                                   Function<? super T, ? extends U> valueMapper) {
            this.keyMapper = keyMapper;
            this.valueMapper = valueMapper;
        }

        @Override
        public Map<K, U> supplier() { return new HashMap<>(); }

        @Override
        public void accumulator(Map<K, U> map, T element) {
            map.put(keyMapper.apply(element), valueMapper.apply(element));
        }

        @Override
        public Map<K, U> finisher(Map<K, U> map) { return Collections.unmodifiableMap(map); }
    }

    private static class ToUnmodifiableListCollector<T> implements Collector<T, List<T>, List<T>> {
        @Override
        public List<T> supplier() { return new ArrayList<>(); }

        @Override
        public void accumulator(List<T> list, T element) { list.add(element); }

        @Override
        public List<T> finisher(List<T> list) { return Collections.unmodifiableList(list); }
    }
}
