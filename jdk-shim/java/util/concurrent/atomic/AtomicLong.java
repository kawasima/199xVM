package java.util.concurrent.atomic;

import java.io.Serializable;
import java.util.function.LongBinaryOperator;
import java.util.function.LongUnaryOperator;

public class AtomicLong extends Number implements Serializable {
    private static final long serialVersionUID = 1927816293512124184L;

    private volatile long value;

    public AtomicLong(long initialValue) {
        this.value = initialValue;
    }

    public AtomicLong() {}

    public final long get() {
        return value;
    }

    public final void set(long newValue) {
        value = newValue;
    }

    public final void lazySet(long newValue) {
        value = newValue;
    }

    public final long getAndSet(long newValue) {
        long prev = value;
        value = newValue;
        return prev;
    }

    public final boolean compareAndSet(long expectedValue, long newValue) {
        if (value == expectedValue) {
            value = newValue;
            return true;
        }
        return false;
    }

    @Deprecated(since="9")
    public final boolean weakCompareAndSet(long expectedValue, long newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetPlain(long expectedValue, long newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final long getAndIncrement() {
        return getAndAdd(1L);
    }

    public final long getAndDecrement() {
        return getAndAdd(-1L);
    }

    public final long getAndAdd(long delta) {
        long prev = value;
        value = prev + delta;
        return prev;
    }

    public final long incrementAndGet() {
        return addAndGet(1L);
    }

    public final long decrementAndGet() {
        return addAndGet(-1L);
    }

    public final long addAndGet(long delta) {
        value = value + delta;
        return value;
    }

    public final long getAndUpdate(LongUnaryOperator updateFunction) {
        long prev = value;
        long next = updateFunction.applyAsLong(prev);
        value = next;
        return prev;
    }

    public final long updateAndGet(LongUnaryOperator updateFunction) {
        long prev = value;
        long next = updateFunction.applyAsLong(prev);
        value = next;
        return next;
    }

    public final long getAndAccumulate(long x, LongBinaryOperator accumulatorFunction) {
        long prev = value;
        long next = accumulatorFunction.applyAsLong(prev, x);
        value = next;
        return prev;
    }

    public final long accumulateAndGet(long x, LongBinaryOperator accumulatorFunction) {
        long prev = value;
        long next = accumulatorFunction.applyAsLong(prev, x);
        value = next;
        return next;
    }

    public int intValue() {
        return (int) value;
    }

    public long longValue() {
        return value;
    }

    public float floatValue() {
        return (float) value;
    }

    public double doubleValue() {
        return (double) value;
    }

    public String toString() {
        return Long.toString(value);
    }
}
