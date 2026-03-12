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
import java.util.HashSet;
import java.util.List;
import java.util.Set;

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
}
