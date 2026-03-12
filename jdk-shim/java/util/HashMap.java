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

package java.util;

import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import java.util.function.Function;

/**
 * HashMap using linear probing with parallel key/value arrays.
 */
public class HashMap<K, V> implements Map<K, V> {
    private static final int DEFAULT_CAPACITY = 16;

    private Object[] keys;
    private Object[] values;
    private int size;

    public HashMap() {
        keys = new Object[DEFAULT_CAPACITY];
        values = new Object[DEFAULT_CAPACITY];
    }

    public HashMap(int initialCapacity) {
        keys = new Object[initialCapacity];
        values = new Object[initialCapacity];
    }

    public HashMap(Map<? extends K, ? extends V> m) {
        this(Math.max(m.size() * 2, DEFAULT_CAPACITY));
        putAll(m);
    }

    public static <K, V> HashMap<K, V> newHashMap(int expectedSize) {
        return new HashMap<>(Math.max(expectedSize, DEFAULT_CAPACITY));
    }

    private int indexOf(Object key) {
        for (int i = 0; i < size; i++) {
            if (Objects.equals(keys[i], key)) return i;
        }
        return -1;
    }

    private void grow() {
        int newCap = keys.length * 2;
        Object[] newKeys = new Object[newCap];
        Object[] newVals = new Object[newCap];
        for (int i = 0; i < size; i++) {
            newKeys[i] = keys[i];
            newVals[i] = values[i];
        }
        keys = newKeys;
        values = newVals;
    }

    @Override
    public int size() { return size; }

    @Override
    public boolean isEmpty() { return size == 0; }

    @Override
    public boolean containsKey(Object key) {
        return indexOf(key) >= 0;
    }

    public boolean containsValue(Object value) {
        for (int i = 0; i < size; i++) {
            if (Objects.equals(values[i], value)) return true;
        }
        return false;
    }

    @Override
    @SuppressWarnings("unchecked")
    public V get(Object key) {
        int i = indexOf(key);
        return i >= 0 ? (V) values[i] : null;
    }

    @SuppressWarnings("unchecked")
    public V getOrDefault(Object key, V defaultValue) {
        int i = indexOf(key);
        return i >= 0 ? (V) values[i] : defaultValue;
    }

    @Override
    @SuppressWarnings("unchecked")
    public V put(K key, V value) {
        int i = indexOf(key);
        if (i >= 0) {
            V old = (V) values[i];
            values[i] = value;
            return old;
        }
        if (size == keys.length) grow();
        keys[size] = key;
        values[size] = value;
        size++;
        return null;
    }

    @SuppressWarnings("unchecked")
    public V putIfAbsent(K key, V value) {
        int i = indexOf(key);
        if (i >= 0) {
            return (V) values[i];
        }
        if (size == keys.length) grow();
        keys[size] = key;
        values[size] = value;
        size++;
        return null;
    }

    @Override
    @SuppressWarnings("unchecked")
    public V remove(Object key) {
        int i = indexOf(key);
        if (i < 0) return null;
        V old = (V) values[i];
        removeAt(i);
        return old;
    }

    public boolean remove(Object key, Object value) {
        int i = indexOf(key);
        if (i < 0 || !Objects.equals(values[i], value)) return false;
        removeAt(i);
        return true;
    }

    private void removeAt(int i) {
        size--;
        keys[i] = keys[size];
        values[i] = values[size];
        keys[size] = null;
        values[size] = null;
    }

    @SuppressWarnings("unchecked")
    public V replace(K key, V value) {
        int i = indexOf(key);
        if (i < 0) return null;
        V old = (V) values[i];
        values[i] = value;
        return old;
    }

    public boolean replace(K key, V oldValue, V newValue) {
        int i = indexOf(key);
        if (i < 0 || !Objects.equals(values[i], oldValue)) return false;
        values[i] = newValue;
        return true;
    }

    public void putAll(Map<? extends K, ? extends V> m) {
        Set<? extends Map.Entry<? extends K, ? extends V>> entries = m.entrySet();
        if (entries != null) {
            for (Map.Entry<? extends K, ? extends V> e : entries) {
                put(e.getKey(), e.getValue());
            }
        }
    }

    public void clear() {
        for (int i = 0; i < size; i++) {
            keys[i] = null;
            values[i] = null;
        }
        size = 0;
    }

    @SuppressWarnings("unchecked")
    public V compute(K key, BiFunction<? super K, ? super V, ? extends V> fn) {
        int i = indexOf(key);
        V oldValue = i >= 0 ? (V) values[i] : null;
        V newValue = fn.apply(key, oldValue);
        if (newValue != null) {
            if (i >= 0) {
                values[i] = newValue;
            } else {
                if (size == keys.length) grow();
                keys[size] = key;
                values[size] = newValue;
                size++;
            }
        } else if (i >= 0) {
            removeAt(i);
        }
        return newValue;
    }

    @SuppressWarnings("unchecked")
    public V computeIfAbsent(K key, Function<? super K, ? extends V> fn) {
        int i = indexOf(key);
        if (i >= 0) {
            return (V) values[i];
        }
        V newValue = fn.apply(key);
        if (newValue != null) {
            if (size == keys.length) grow();
            keys[size] = key;
            values[size] = newValue;
            size++;
        }
        return newValue;
    }

    @SuppressWarnings("unchecked")
    public V computeIfPresent(K key, BiFunction<? super K, ? super V, ? extends V> fn) {
        int i = indexOf(key);
        if (i < 0) return null;
        V oldValue = (V) values[i];
        if (oldValue == null) return null;
        V newValue = fn.apply(key, oldValue);
        if (newValue != null) {
            values[i] = newValue;
        } else {
            removeAt(i);
        }
        return newValue;
    }

    @SuppressWarnings("unchecked")
    public V merge(K key, V value, BiFunction<? super V, ? super V, ? extends V> fn) {
        int i = indexOf(key);
        V oldValue = i >= 0 ? (V) values[i] : null;
        V newValue = (oldValue == null) ? value : fn.apply(oldValue, value);
        if (newValue != null) {
            if (i >= 0) {
                values[i] = newValue;
            } else {
                if (size == keys.length) grow();
                keys[size] = key;
                values[size] = newValue;
                size++;
            }
        } else if (i >= 0) {
            removeAt(i);
        }
        return newValue;
    }

    @SuppressWarnings("unchecked")
    public void forEach(BiConsumer<? super K, ? super V> action) {
        for (int i = 0; i < size; i++) {
            action.accept((K) keys[i], (V) values[i]);
        }
    }

    @SuppressWarnings("unchecked")
    public void replaceAll(BiFunction<? super K, ? super V, ? extends V> fn) {
        for (int i = 0; i < size; i++) {
            values[i] = fn.apply((K) keys[i], (V) values[i]);
        }
    }

    // ---- keySet / values / entrySet ----

    @Override
    public Set<K> keySet() {
        return new KeySet();
    }

    @Override
    public Collection<V> values() {
        return new Values();
    }

    @Override
    public Set<Entry<K, V>> entrySet() {
        return new EntrySet();
    }

    private class KeySet implements Set<K> {
        @Override
        public int size() { return size; }

        @Override
        public boolean isEmpty() { return size == 0; }

        @Override
        public boolean contains(Object o) { return containsKey(o); }

        @Override
        public boolean add(K e) {
            throw new UnsupportedOperationException();
        }

        @Override
        public boolean addAll(Collection<? extends K> c) {
            throw new UnsupportedOperationException();
        }

        @Override
        public void clear() { HashMap.this.clear(); }

        @Override
        public Iterator<K> iterator() {
            return new Iterator<K>() {
                int cursor = 0;

                @Override
                public boolean hasNext() { return cursor < size; }

                @Override
                @SuppressWarnings("unchecked")
                public K next() {
                    if (cursor >= size) throw new NoSuchElementException();
                    return (K) keys[cursor++];
                }
            };
        }
    }

    private class Values implements Collection<V> {
        @Override
        public int size() { return size; }

        @Override
        public boolean isEmpty() { return size == 0; }

        @Override
        public boolean contains(Object o) { return containsValue(o); }

        @Override
        public boolean add(V e) {
            throw new UnsupportedOperationException();
        }

        @Override
        public boolean addAll(Collection<? extends V> c) {
            throw new UnsupportedOperationException();
        }

        @Override
        public void clear() { HashMap.this.clear(); }

        @Override
        public Iterator<V> iterator() {
            return new Iterator<V>() {
                int cursor = 0;

                @Override
                public boolean hasNext() { return cursor < size; }

                @Override
                @SuppressWarnings("unchecked")
                public V next() {
                    if (cursor >= size) throw new NoSuchElementException();
                    return (V) values[cursor++];
                }
            };
        }
    }

    private class EntrySet implements Set<Entry<K, V>> {
        @Override
        public int size() { return size; }

        @Override
        public boolean isEmpty() { return size == 0; }

        @Override
        public boolean contains(Object o) {
            if (!(o instanceof Map.Entry)) return false;
            Map.Entry<?, ?> e = (Map.Entry<?, ?>) o;
            int i = indexOf(e.getKey());
            return i >= 0 && Objects.equals(values[i], e.getValue());
        }

        @Override
        public boolean add(Entry<K, V> e) {
            throw new UnsupportedOperationException();
        }

        @Override
        public boolean addAll(Collection<? extends Entry<K, V>> c) {
            throw new UnsupportedOperationException();
        }

        @Override
        public void clear() { HashMap.this.clear(); }

        @Override
        public Iterator<Entry<K, V>> iterator() {
            return new Iterator<Entry<K, V>>() {
                int cursor = 0;

                @Override
                public boolean hasNext() { return cursor < size; }

                @Override
                public Entry<K, V> next() {
                    if (cursor >= size) throw new NoSuchElementException();
                    int idx = cursor++;
                    return new SimpleEntry<>(idx);
                }
            };
        }
    }

    private class SimpleEntry<K2, V2> implements Map.Entry<K2, V2> {
        private final int index;

        SimpleEntry(int index) {
            this.index = index;
        }

        @Override
        @SuppressWarnings("unchecked")
        public K2 getKey() { return (K2) keys[index]; }

        @Override
        @SuppressWarnings("unchecked")
        public V2 getValue() { return (V2) values[index]; }

        @Override
        @SuppressWarnings("unchecked")
        public V2 setValue(V2 value) {
            V2 old = (V2) values[index];
            values[index] = value;
            return old;
        }

        @Override
        public String toString() {
            return getKey() + "=" + getValue();
        }
    }

    // ---- Object methods ----

    @Override
    @SuppressWarnings("unchecked")
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Map)) return false;
        Map<?, ?> m = (Map<?, ?>) o;
        if (m.size() != size) return false;
        for (int i = 0; i < size; i++) {
            Object k = keys[i];
            Object v = values[i];
            Object mv = m.get(k);
            if (!Objects.equals(v, mv)) return false;
            if (mv == null && !m.containsKey(k)) return false;
        }
        return true;
    }

    @Override
    public int hashCode() {
        int h = 0;
        for (int i = 0; i < size; i++) {
            int kh = keys[i] == null ? 0 : keys[i].hashCode();
            int vh = values[i] == null ? 0 : values[i].hashCode();
            h += kh ^ vh;
        }
        return h;
    }

    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder("{");
        for (int i = 0; i < size; i++) {
            if (i > 0) sb.append(", ");
            sb.append(keys[i] == this ? "(this Map)" : keys[i]);
            sb.append("=");
            sb.append(values[i] == this ? "(this Map)" : values[i]);
        }
        sb.append("}");
        return sb.toString();
    }
}
