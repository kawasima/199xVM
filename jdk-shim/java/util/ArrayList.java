package java.util;

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
        int newCapacity = elementData.length * 2 + 1;
        Object[] newData = new Object[newCapacity];
        for (int i = 0; i < size; i++) {
            newData[i] = elementData[i];
        }
        elementData = newData;
    }

    @Override
    public int size() { return size; }

    @Override
    public boolean isEmpty() { return size == 0; }

    @Override
    public boolean contains(Object o) {
        for (int i = 0; i < size; i++) {
            if (Objects.equals(o, elementData[i])) return true;
        }
        return false;
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
        if (size == elementData.length) grow();
        for (int i = size; i > index; i--) {
            elementData[i] = elementData[i - 1];
        }
        elementData[index] = element;
        size++;
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
    public boolean addAll(Collection<? extends E> c) {
        for (E e : c) {
            add(e);
        }
        return true;
    }

    @Override
    public void clear() {
        for (int i = 0; i < size; i++) elementData[i] = null;
        size = 0;
    }

    @Override
    public Iterator<E> iterator() {
        return new Itr();
    }

    private class Itr implements Iterator<E> {
        int cursor = 0;

        @Override
        public boolean hasNext() { return cursor < size; }

        @Override
        @SuppressWarnings("unchecked")
        public E next() { return (E) elementData[cursor++]; }
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
