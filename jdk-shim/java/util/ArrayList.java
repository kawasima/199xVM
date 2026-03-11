package java.util;

import java.util.function.Consumer;
import java.util.function.Predicate;
import java.util.function.UnaryOperator;

public class ArrayList<E> implements List<E> {
    private Object[] elementData;
    private int size;

    public ArrayList() {
        elementData = new Object[10];
    }

    public ArrayList(int initialCapacity) {
        elementData = new Object[initialCapacity];
    }

    public ArrayList(Collection<? extends E> c) {
        this();
        addAll(c);
    }

    private void grow() {
        grow(size + 1);
    }

    private void grow(int minCapacity) {
        int newCapacity = elementData.length * 2 + 1;
        if (newCapacity < minCapacity) {
            newCapacity = minCapacity;
        }
        Object[] newData = new Object[newCapacity];
        for (int i = 0; i < size; i++) {
            newData[i] = elementData[i];
        }
        elementData = newData;
    }

    public void ensureCapacity(int minCapacity) {
        if (minCapacity > elementData.length) {
            grow(minCapacity);
        }
    }

    public void trimToSize() {
        if (size < elementData.length) {
            Object[] newData = new Object[size];
            for (int i = 0; i < size; i++) {
                newData[i] = elementData[i];
            }
            elementData = newData;
        }
    }

    @Override
    public int size() { return size; }

    @Override
    public boolean isEmpty() { return size == 0; }

    @Override
    public boolean contains(Object o) {
        return indexOf(o) >= 0;
    }

    public int indexOf(Object o) {
        for (int i = 0; i < size; i++) {
            if (Objects.equals(o, elementData[i])) return i;
        }
        return -1;
    }

    public int lastIndexOf(Object o) {
        for (int i = size - 1; i >= 0; i--) {
            if (Objects.equals(o, elementData[i])) return i;
        }
        return -1;
    }

    @Override
    @SuppressWarnings("unchecked")
    public E get(int index) {
        if (index < 0 || index >= size) throw new IndexOutOfBoundsException();
        return (E) elementData[index];
    }

    @Override
    @SuppressWarnings("unchecked")
    public E set(int index, E element) {
        if (index < 0 || index >= size) throw new IndexOutOfBoundsException();
        E old = (E) elementData[index];
        elementData[index] = element;
        return old;
    }

    @Override
    public boolean add(E e) {
        if (size == elementData.length) grow();
        elementData[size++] = e;
        return true;
    }

    @Override
    public void add(int index, E element) {
        if (index < 0 || index > size) throw new IndexOutOfBoundsException();
        if (size == elementData.length) grow();
        for (int i = size; i > index; i--) {
            elementData[i] = elementData[i - 1];
        }
        elementData[index] = element;
        size++;
    }

    @Override
    public boolean remove(Object o) {
        int i = indexOf(o);
        if (i < 0) return false;
        remove(i);
        return true;
    }

    @SuppressWarnings("unchecked")
    public E remove(int index) {
        if (index < 0 || index >= size) throw new IndexOutOfBoundsException();
        E old = (E) elementData[index];
        for (int i = index; i < size - 1; i++) {
            elementData[i] = elementData[i + 1];
        }
        elementData[--size] = null;
        return old;
    }

    @Override
    public Object[] toArray() {
        Object[] result = new Object[size];
        for (int i = 0; i < size; i++) {
            result[i] = elementData[i];
        }
        return result;
    }

    @Override
    @SuppressWarnings("unchecked")
    public <T> T[] toArray(T[] a) {
        // If the supplied array is too small, just copy into it up to its length.
        // This VM doesn't support reflection-based array creation.
        for (int i = 0; i < size && i < a.length; i++) {
            a[i] = (T) elementData[i];
        }
        if (a.length > size) {
            a[size] = null;
        }
        return a;
    }

    // JDK 21+ SequencedCollection methods

    public void addFirst(E element) {
        add(0, element);
    }

    public void addLast(E element) {
        add(element);
    }

    @SuppressWarnings("unchecked")
    public E getFirst() {
        if (size == 0) throw new NoSuchElementException();
        return (E) elementData[0];
    }

    @SuppressWarnings("unchecked")
    public E getLast() {
        if (size == 0) throw new NoSuchElementException();
        return (E) elementData[size - 1];
    }

    public E removeFirst() {
        if (size == 0) throw new NoSuchElementException();
        return remove(0);
    }

    public E removeLast() {
        if (size == 0) throw new NoSuchElementException();
        return remove(size - 1);
    }

    @Override
    public boolean addAll(Collection<? extends E> c) {
        for (E e : c) {
            add(e);
        }
        return true;
    }

    @Override
    public boolean addAll(int index, Collection<? extends E> c) {
        if (index < 0 || index > size) throw new IndexOutOfBoundsException();
        Object[] a = c.toArray();
        int numNew = a.length;
        if (numNew == 0) return false;
        ensureCapacity(size + numNew);
        // shift existing elements to the right
        for (int i = size - 1; i >= index; i--) {
            elementData[i + numNew] = elementData[i];
        }
        // copy new elements into the gap
        for (int i = 0; i < numNew; i++) {
            elementData[index + i] = a[i];
        }
        size += numNew;
        return true;
    }

    @Override
    public boolean containsAll(Collection<?> c) {
        for (Object e : c) {
            if (!contains(e)) return false;
        }
        return true;
    }

    @Override
    public boolean removeAll(Collection<?> c) {
        boolean modified = false;
        for (int i = size - 1; i >= 0; i--) {
            if (c.contains(elementData[i])) {
                remove(i);
                modified = true;
            }
        }
        return modified;
    }

    @Override
    public boolean retainAll(Collection<?> c) {
        boolean modified = false;
        for (int i = size - 1; i >= 0; i--) {
            if (!c.contains(elementData[i])) {
                remove(i);
                modified = true;
            }
        }
        return modified;
    }

    public void forEach(Consumer<? super E> action) {
        Objects.requireNonNull(action);
        for (int i = 0; i < size; i++) {
            @SuppressWarnings("unchecked")
            E e = (E) elementData[i];
            action.accept(e);
        }
    }

    public boolean removeIf(Predicate<? super E> filter) {
        Objects.requireNonNull(filter);
        boolean modified = false;
        for (int i = size - 1; i >= 0; i--) {
            @SuppressWarnings("unchecked")
            E e = (E) elementData[i];
            if (filter.test(e)) {
                remove(i);
                modified = true;
            }
        }
        return modified;
    }

    @SuppressWarnings("unchecked")
    public void sort(Comparator<? super E> c) {
        // Simple insertion sort
        for (int i = 1; i < size; i++) {
            E key = (E) elementData[i];
            int j = i - 1;
            while (j >= 0 && c.compare((E) elementData[j], key) > 0) {
                elementData[j + 1] = elementData[j];
                j--;
            }
            elementData[j + 1] = key;
        }
    }

    @SuppressWarnings("unchecked")
    public void replaceAll(UnaryOperator<E> operator) {
        Objects.requireNonNull(operator);
        for (int i = 0; i < size; i++) {
            elementData[i] = operator.apply((E) elementData[i]);
        }
    }

    @Override
    public void clear() {
        for (int i = 0; i < size; i++) elementData[i] = null;
        size = 0;
    }

    @Override
    public boolean equals(Object o) {
        if (o == this) return true;
        if (!(o instanceof List)) return false;
        List<?> other = (List<?>) o;
        if (other.size() != size) return false;
        Iterator<?> it = other.iterator();
        for (int i = 0; i < size; i++) {
            if (!it.hasNext()) return false;
            if (!Objects.equals(elementData[i], it.next())) return false;
        }
        return true;
    }

    @Override
    public int hashCode() {
        int hashCode = 1;
        for (int i = 0; i < size; i++) {
            Object e = elementData[i];
            hashCode = 31 * hashCode + (e == null ? 0 : e.hashCode());
        }
        return hashCode;
    }

    @Override
    public Iterator<E> iterator() {
        return new Itr();
    }

    @Override
    public ListIterator<E> listIterator() {
        return new ListItr(0);
    }

    @Override
    public ListIterator<E> listIterator(int index) {
        return new ListItr(index);
    }

    @Override
    public List<E> subList(int fromIndex, int toIndex) {
        if (fromIndex < 0 || toIndex > size || fromIndex > toIndex) throw new IndexOutOfBoundsException();
        ArrayList<E> sub = new ArrayList<>(toIndex - fromIndex);
        for (int i = fromIndex; i < toIndex; i++) {
            @SuppressWarnings("unchecked") E e = (E) elementData[i];
            sub.add(e);
        }
        return sub;
    }

    private class Itr implements Iterator<E> {
        int cursor = 0;

        @Override
        public boolean hasNext() { return cursor < size; }

        @Override
        @SuppressWarnings("unchecked")
        public E next() { return (E) elementData[cursor++]; }
    }

    private class ListItr extends Itr implements ListIterator<E> {
        ListItr(int index) { this.cursor = index; }
        @Override public boolean hasPrevious() { return cursor > 0; }
        @Override @SuppressWarnings("unchecked") public E previous() { return (E) elementData[--cursor]; }
        @Override public int nextIndex() { return cursor; }
        @Override public int previousIndex() { return cursor - 1; }
        @Override public void set(E e) {
            if (cursor <= 0 || cursor > size) throw new IndexOutOfBoundsException();
            elementData[cursor - 1] = e;
        }
        @Override public void add(E e) { ArrayList.this.add(cursor++, e); }
    }

    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < size; i++) {
            if (i > 0) sb.append(", ");
            sb.append(elementData[i]);
        }
        sb.append("]");
        return sb.toString();
    }
}
