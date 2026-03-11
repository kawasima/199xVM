package java.util.concurrent.atomic;

import java.io.Serializable;
import java.util.function.BinaryOperator;
import java.util.function.UnaryOperator;

public class AtomicReferenceArray<E> implements Serializable {
    private static final long serialVersionUID = -6209656149925076980L;

    private final Object[] array;

    public AtomicReferenceArray(int length) {
        this.array = new Object[length];
    }

    public AtomicReferenceArray(E[] array) {
        this.array = new Object[array.length];
        for (int i = 0; i < array.length; i++) {
            this.array[i] = array[i];
        }
    }

    public final int length() {
        return array.length;
    }

    @SuppressWarnings("unchecked")
    public final E get(int i) {
        return (E) array[i];
    }

    public final void set(int i, E newValue) {
        array[i] = newValue;
    }

    public final void lazySet(int i, E newValue) {
        array[i] = newValue;
    }

    @SuppressWarnings("unchecked")
    public final E getAndSet(int i, E newValue) {
        E prev = (E) array[i];
        array[i] = newValue;
        return prev;
    }

    public final boolean compareAndSet(int i, E expectedValue, E newValue) {
        Object cur = array[i];
        if (cur == expectedValue) {
            array[i] = newValue;
            return true;
        }
        return false;
    }

    @Deprecated(since="9")
    public final boolean weakCompareAndSet(int i, E expectedValue, E newValue) {
        return compareAndSet(i, expectedValue, newValue);
    }

    public final boolean weakCompareAndSetPlain(int i, E expectedValue, E newValue) {
        return compareAndSet(i, expectedValue, newValue);
    }

    @SuppressWarnings("unchecked")
    public final E getAndUpdate(int i, UnaryOperator<E> updateFunction) {
        E prev = (E) array[i];
        E next = updateFunction.apply(prev);
        array[i] = next;
        return prev;
    }

    @SuppressWarnings("unchecked")
    public final E updateAndGet(int i, UnaryOperator<E> updateFunction) {
        E prev = (E) array[i];
        E next = updateFunction.apply(prev);
        array[i] = next;
        return next;
    }

    @SuppressWarnings("unchecked")
    public final E getAndAccumulate(int i, E x, BinaryOperator<E> accumulatorFunction) {
        E prev = (E) array[i];
        E next = accumulatorFunction.apply(prev, x);
        array[i] = next;
        return prev;
    }

    @SuppressWarnings("unchecked")
    public final E accumulateAndGet(int i, E x, BinaryOperator<E> accumulatorFunction) {
        E prev = (E) array[i];
        E next = accumulatorFunction.apply(prev, x);
        array[i] = next;
        return next;
    }

    @SuppressWarnings("unchecked")
    public final E getPlain(int i) {
        return (E) array[i];
    }

    public final void setPlain(int i, E newValue) {
        array[i] = newValue;
    }

    @SuppressWarnings("unchecked")
    public final E getOpaque(int i) {
        return (E) array[i];
    }

    public final void setOpaque(int i, E newValue) {
        array[i] = newValue;
    }

    @SuppressWarnings("unchecked")
    public final E getAcquire(int i) {
        return (E) array[i];
    }

    public final void setRelease(int i, E newValue) {
        array[i] = newValue;
    }

    @SuppressWarnings("unchecked")
    public final E compareAndExchange(int i, E expectedValue, E newValue) {
        E prev = (E) array[i];
        if (prev == expectedValue) {
            array[i] = newValue;
        }
        return prev;
    }

    public final E compareAndExchangeAcquire(int i, E expectedValue, E newValue) {
        return compareAndExchange(i, expectedValue, newValue);
    }

    public final E compareAndExchangeRelease(int i, E expectedValue, E newValue) {
        return compareAndExchange(i, expectedValue, newValue);
    }

    public final boolean weakCompareAndSetVolatile(int i, E expectedValue, E newValue) {
        return compareAndSet(i, expectedValue, newValue);
    }

    public final boolean weakCompareAndSetAcquire(int i, E expectedValue, E newValue) {
        return compareAndSet(i, expectedValue, newValue);
    }

    public final boolean weakCompareAndSetRelease(int i, E expectedValue, E newValue) {
        return compareAndSet(i, expectedValue, newValue);
    }

    public String toString() {
        StringBuilder sb = new StringBuilder();
        sb.append('[');
        for (int i = 0; i < array.length; i++) {
            if (i > 0) sb.append(", ");
            sb.append(String.valueOf(array[i]));
        }
        sb.append(']');
        return sb.toString();
    }
}
