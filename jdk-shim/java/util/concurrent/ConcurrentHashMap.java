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

package java.util.concurrent;

import java.io.Serializable;
import java.util.AbstractMap;
import java.util.Collection;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.function.BiFunction;
import java.util.function.Function;

public class ConcurrentHashMap<K, V> extends AbstractMap<K, V> implements ConcurrentMap<K, V>, Serializable {
    private static final long serialVersionUID = 7249069246763182397L;

    private final HashMap<K, V> map;

    public ConcurrentHashMap() {
        this.map = new HashMap<>();
    }

    public ConcurrentHashMap(int initialCapacity) {
        this.map = new HashMap<>(initialCapacity);
    }

    public ConcurrentHashMap(Map<? extends K, ? extends V> m) {
        this.map = new HashMap<>(m.size());
        this.map.putAll(m);
    }

    public ConcurrentHashMap(int initialCapacity, float loadFactor) {
        this.map = new HashMap<>(initialCapacity);
    }

    public ConcurrentHashMap(int initialCapacity, float loadFactor, int concurrencyLevel) {
        this.map = new HashMap<>(initialCapacity);
    }

    public V get(Object key) {
        return map.get(key);
    }

    public V put(K key, V value) {
        return map.put(key, value);
    }

    public void putAll(Map<? extends K, ? extends V> m) {
        map.putAll(m);
    }

    public V remove(Object key) {
        return map.remove(key);
    }

    public boolean remove(Object key, Object value) {
        V cur = map.get(key);
        if (cur == null) return false;
        if (!cur.equals(value)) return false;
        map.remove(key);
        return true;
    }

    public V putIfAbsent(K key, V value) {
        V cur = map.get(key);
        if (cur == null) {
            map.put(key, value);
            return null;
        }
        return cur;
    }

    public boolean replace(K key, V oldValue, V newValue) {
        V cur = map.get(key);
        if (cur == null) return false;
        if (!cur.equals(oldValue)) return false;
        map.put(key, newValue);
        return true;
    }

    public V replace(K key, V value) {
        if (!map.containsKey(key)) return null;
        return map.put(key, value);
    }

    public V computeIfAbsent(K key, Function<? super K, ? extends V> mappingFunction) {
        V cur = map.get(key);
        if (cur == null) {
            V next = mappingFunction.apply(key);
            if (next != null) {
                map.put(key, next);
            }
            return next;
        }
        return cur;
    }

    public V computeIfPresent(K key, BiFunction<? super K, ? super V, ? extends V> remappingFunction) {
        V cur = map.get(key);
        if (cur == null) return null;
        V next = remappingFunction.apply(key, cur);
        if (next == null) {
            map.remove(key);
            return null;
        }
        map.put(key, next);
        return next;
    }

    public V compute(K key, BiFunction<? super K, ? super V, ? extends V> remappingFunction) {
        V cur = map.get(key);
        V next = remappingFunction.apply(key, cur);
        if (next == null) {
            map.remove(key);
            return null;
        }
        map.put(key, next);
        return next;
    }

    public V merge(K key, V value, BiFunction<? super V, ? super V, ? extends V> remappingFunction) {
        V cur = map.get(key);
        if (cur == null) {
            map.put(key, value);
            return value;
        }
        V next = remappingFunction.apply(cur, value);
        if (next == null) {
            map.remove(key);
            return null;
        }
        map.put(key, next);
        return next;
    }

    public void clear() {
        map.clear();
    }

    public boolean containsKey(Object key) {
        return map.containsKey(key);
    }

    public boolean containsValue(Object value) {
        return map.containsValue(value);
    }

    public boolean isEmpty() {
        return map.isEmpty();
    }

    public int size() {
        return map.size();
    }

    public Set<K> keySet() {
        return map.keySet();
    }

    public Collection<V> values() {
        return map.values();
    }

    public Set<Entry<K, V>> entrySet() {
        return map.entrySet();
    }
}
