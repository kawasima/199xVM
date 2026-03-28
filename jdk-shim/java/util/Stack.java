package java.util;

public class Stack<E> extends ArrayList<E> {
    private static final long serialVersionUID = 1224463164541339165L;

    public Stack() {
        super();
    }

    public E push(E item) {
        add(item);
        return item;
    }

    public synchronized E pop() {
        int len = size();
        E obj = peek();
        remove(len - 1);
        return obj;
    }

    public synchronized E peek() {
        int len = size();
        if (len == 0) {
            throw new EmptyStackException();
        }
        return get(len - 1);
    }

    public boolean empty() {
        return isEmpty();
    }

    public synchronized int search(Object o) {
        int index = lastIndexOf(o);
        return index >= 0 ? size() - index : -1;
    }
}
