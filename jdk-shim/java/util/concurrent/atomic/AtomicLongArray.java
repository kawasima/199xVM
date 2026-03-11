package java.util.concurrent.atomic;

import java.io.Serializable;
import java.util.function.LongBinaryOperator;
import java.util.function.LongUnaryOperator;

public class AtomicLongArray implements Serializable {
    private static final long serialVersionUID = -2308431214976778248L;

    private final long[] array;

    public AtomicLongArray(int length) {
        this.array = new long[length];
    }

    public AtomicLongArray(long[] array) {
        this.array = new long[array.length];
        for (int i = 0; i < array.length; i++) {
            this.array[i] = array[i];
        }
    }

    public final int length() {
        return array.length;
    }

    public final long get(int i) {
        return array[i];
    }

    public final void set(int i, long newValue) {
        array[i] = newValue;
    }

    public final void lazySet(int i, long newValue) {
        array[i] = newValue;
    }

    public final long getAndSet(int i, long newValue) {
        long prev = array[i];
        array[i] = newValue;
        return prev;
    }

    public final boolean compareAndSet(int i, long expectedValue, long newValue) {
        long cur = array[i];
        if (cur == expectedValue) {
            array[i] = newValue;
            return true;
        }
        return false;
    }

    @Deprecated(since="9")
    public final boolean weakCompareAndSet(int i, long expectedValue, long newValue) {
        return compareAndSet(i, expectedValue, newValue);
    }

    public final boolean weakCompareAndSetPlain(int i, long expectedValue, long newValue) {
        return compareAndSet(i, expectedValue, newValue);
    }

    public final long getAndIncrement(int i) {
        return getAndAdd(i, 1L);
    }

    public final long getAndDecrement(int i) {
        return getAndAdd(i, -1L);
    }

    public final long getAndAdd(int i, long delta) {
        long prev = array[i];
        array[i] = prev + delta;
        return prev;
    }

    public final long incrementAndGet(int i) {
        return addAndGet(i, 1L);
    }

    public final long decrementAndGet(int i) {
        return addAndGet(i, -1L);
    }

    public final long addAndGet(int i, long delta) {
        long next = array[i] + delta;
        array[i] = next;
        return next;
    }

    public final long getAndUpdate(int i, LongUnaryOperator updateFunction) {
        long prev = array[i];
        long next = updateFunction.applyAsLong(prev);
        array[i] = next;
        return prev;
    }

    public final long updateAndGet(int i, LongUnaryOperator updateFunction) {
        long prev = array[i];
        long next = updateFunction.applyAsLong(prev);
        array[i] = next;
        return next;
    }

    public final long getAndAccumulate(int i, long x, LongBinaryOperator accumulatorFunction) {
        long prev = array[i];
        long next = accumulatorFunction.applyAsLong(prev, x);
        array[i] = next;
        return prev;
    }

    public final long accumulateAndGet(int i, long x, LongBinaryOperator accumulatorFunction) {
        long prev = array[i];
        long next = accumulatorFunction.applyAsLong(prev, x);
        array[i] = next;
        return next;
    }

    public String toString() {
        StringBuilder sb = new StringBuilder();
        sb.append('[');
        for (int i = 0; i < array.length; i++) {
            if (i > 0) sb.append(", ");
            sb.append(array[i]);
        }
        sb.append(']');
        return sb.toString();
    }
}
