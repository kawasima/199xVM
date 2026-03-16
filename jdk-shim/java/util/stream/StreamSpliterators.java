/*
 * Copyright (c) 2012, 2024, Oracle and/or its affiliates. All rights reserved.
 * ORACLE PROPRIETARY/CONFIDENTIAL. Use is subject to license terms.
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 */
package java.util.stream;

import java.util.Objects;
import java.util.Spliterator;
import java.util.function.Consumer;
import java.util.function.DoubleConsumer;
import java.util.function.DoubleSupplier;
import java.util.function.IntConsumer;
import java.util.function.IntSupplier;
import java.util.function.LongConsumer;
import java.util.function.LongSupplier;
import java.util.function.Supplier;

/**
 * Spliterator implementations for wrapping and delegating spliterators, used
 * in the implementation of the {@link Stream#spliterator()} method.
 *
 * @since 1.8
 */
class StreamSpliterators {

    /**
     * A Spliterator that infinitely supplies elements in no particular order.
     *
     * <p>Splitting divides the estimated size in two and stops when the
     * estimate size is 0.
     *
     * <p>The {@code forEachRemaining} method if invoked will never terminate.
     * The {@code tryAdvance} method always returns true.
     *
     */
    abstract static class InfiniteSupplyingSpliterator<T> implements Spliterator<T> {
        long estimate;

        protected InfiniteSupplyingSpliterator(long estimate) {
            this.estimate = estimate;
        }

        @Override
        public long estimateSize() {
            return estimate;
        }

        @Override
        public int characteristics() {
            return IMMUTABLE;
        }

        static final class OfRef<T> extends InfiniteSupplyingSpliterator<T> {
            final Supplier<? extends T> s;

            OfRef(long size, Supplier<? extends T> s) {
                super(size);
                this.s = s;
            }

            @Override
            public boolean tryAdvance(Consumer<? super T> action) {
                Objects.requireNonNull(action);

                action.accept(s.get());
                return true;
            }

            @Override
            public Spliterator<T> trySplit() {
                if (estimate == 0)
                    return null;
                return new InfiniteSupplyingSpliterator.OfRef<>(estimate >>>= 1, s);
            }
        }

        static final class OfInt extends InfiniteSupplyingSpliterator<Integer>
                implements Spliterator.OfInt {
            final IntSupplier s;

            OfInt(long size, IntSupplier s) {
                super(size);
                this.s = s;
            }

            @Override
            public boolean tryAdvance(IntConsumer action) {
                Objects.requireNonNull(action);

                action.accept(s.getAsInt());
                return true;
            }

            @Override
            public Spliterator.OfInt trySplit() {
                if (estimate == 0)
                    return null;
                return new InfiniteSupplyingSpliterator.OfInt(estimate = estimate >>> 1, s);
            }
        }

        static final class OfLong extends InfiniteSupplyingSpliterator<Long>
                implements Spliterator.OfLong {
            final LongSupplier s;

            OfLong(long size, LongSupplier s) {
                super(size);
                this.s = s;
            }

            @Override
            public boolean tryAdvance(LongConsumer action) {
                Objects.requireNonNull(action);

                action.accept(s.getAsLong());
                return true;
            }

            @Override
            public Spliterator.OfLong trySplit() {
                if (estimate == 0)
                    return null;
                return new InfiniteSupplyingSpliterator.OfLong(estimate = estimate >>> 1, s);
            }
        }

        static final class OfDouble extends InfiniteSupplyingSpliterator<Double>
                implements Spliterator.OfDouble {
            final DoubleSupplier s;

            OfDouble(long size, DoubleSupplier s) {
                super(size);
                this.s = s;
            }

            @Override
            public boolean tryAdvance(DoubleConsumer action) {
                Objects.requireNonNull(action);

                action.accept(s.getAsDouble());
                return true;
            }

            @Override
            public Spliterator.OfDouble trySplit() {
                if (estimate == 0)
                    return null;
                return new InfiniteSupplyingSpliterator.OfDouble(estimate = estimate >>> 1, s);
            }
        }
    }
}
