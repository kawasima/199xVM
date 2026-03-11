package java.util.concurrent.atomic;

import java.util.function.BinaryOperator;
import java.util.function.UnaryOperator;

/**
 * Shim implementation of AtomicReference with Java 25-compatible public API.
 * The VM is single-threaded today, so atomic operations are implemented with
 * simple compare/update semantics on a volatile field.
 */
public class AtomicReference<V> implements java.io.Serializable {
    private static final long serialVersionUID = -1848883965231344442L;

    @SuppressWarnings("serial")
    private volatile V value;

    public AtomicReference(V initialValue) {
        value = initialValue;
    }

    public AtomicReference() {}

    public final V get() {
        return value;
    }

    public final void set(V newValue) {
        value = newValue;
    }

    public final void lazySet(V newValue) {
        value = newValue;
    }

    public final boolean compareAndSet(V expectedValue, V newValue) {
        if (value == expectedValue) {
            value = newValue;
            return true;
        }
        return false;
    }

    @Deprecated(since="9")
    public final boolean weakCompareAndSet(V expectedValue, V newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetPlain(V expectedValue, V newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final V getAndSet(V newValue) {
        V prev = value;
        value = newValue;
        return prev;
    }

    public final V getAndUpdate(UnaryOperator<V> updateFunction) {
        V prev = get();
        V next = updateFunction.apply(prev);
        set(next);
        return prev;
    }

    public final V updateAndGet(UnaryOperator<V> updateFunction) {
        V prev = get();
        V next = updateFunction.apply(prev);
        set(next);
        return next;
    }

    public final V getAndAccumulate(V x, BinaryOperator<V> accumulatorFunction) {
        V prev = get();
        V next = accumulatorFunction.apply(prev, x);
        set(next);
        return prev;
    }

    public final V accumulateAndGet(V x, BinaryOperator<V> accumulatorFunction) {
        V prev = get();
        V next = accumulatorFunction.apply(prev, x);
        set(next);
        return next;
    }

    public String toString() {
        return String.valueOf(get());
    }

    public final V getPlain() {
        return value;
    }

    public final void setPlain(V newValue) {
        value = newValue;
    }

    public final V getOpaque() {
        return value;
    }

    public final void setOpaque(V newValue) {
        value = newValue;
    }

    public final V getAcquire() {
        return value;
    }

    public final void setRelease(V newValue) {
        value = newValue;
    }

    public final V compareAndExchange(V expectedValue, V newValue) {
        V prev = value;
        if (prev == expectedValue) {
            value = newValue;
        }
        return prev;
    }

    public final V compareAndExchangeAcquire(V expectedValue, V newValue) {
        return compareAndExchange(expectedValue, newValue);
    }

    public final V compareAndExchangeRelease(V expectedValue, V newValue) {
        return compareAndExchange(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetVolatile(V expectedValue, V newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetAcquire(V expectedValue, V newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetRelease(V expectedValue, V newValue) {
        return compareAndSet(expectedValue, newValue);
    }
}
