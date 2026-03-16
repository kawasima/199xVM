/*
 * Copyright (c) 2015, 2023, Oracle and/or its affiliates. All rights reserved.
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

import java.util.Comparator;
import java.util.Spliterator;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Consumer;
import java.util.function.DoubleConsumer;
import java.util.function.DoublePredicate;
import java.util.function.IntConsumer;
import java.util.function.IntPredicate;
import java.util.function.LongConsumer;
import java.util.function.LongPredicate;
import java.util.function.Predicate;

/**
 * Factory for instances of a takeWhile and dropWhile operations
 * that produce subsequences of their input stream.
 *
 * @since 9
 */
final class WhileOps {

    static final int TAKE_FLAGS = StreamOpFlag.NOT_SIZED | StreamOpFlag.IS_SHORT_CIRCUIT;

    static final int DROP_FLAGS = StreamOpFlag.NOT_SIZED;

    /**
     * An unordered spliterator for takeWhile/dropWhile operations.
     *
     * <p>Concrete subtypes of this spliterator (specifically the Taking and
     * Dropping spliterators) are used by the default implementations of
     * takeWhile/dropWhile in Stream.java.
     *
     * @param <T> the type of elements returned by this spliterator
     * @param <T_SPLITR> the type of the spliterator
     */
    abstract static class UnorderedWhileSpliterator<T, T_SPLITR extends Spliterator<T>> implements Spliterator<T> {
        // Power of two constant minus one used for modulus of count
        static final int CANCEL_CHECK_COUNT = (1 << 6) - 1;

        // The underlying spliterator
        final T_SPLITR s;
        // True if no splitting should be performed, if true then
        // this spliterator may be used for an underlying spliterator whose
        // covered elements have an encounter order
        // See use in stream take/dropWhile default methods
        final boolean noSplitting;
        // True when operations are cancelled for all related spliterators
        // For taking, spliterators cannot split or traversed
        // For dropping, spliterators cannot be traversed
        final AtomicBoolean cancel;
        // True while taking or dropping should be performed when traversing
        boolean takeOrDrop = true;
        // The count of elements traversed
        int count;

        UnorderedWhileSpliterator(T_SPLITR s, boolean noSplitting) {
            this.s = s;
            this.noSplitting = noSplitting;
            this.cancel = new AtomicBoolean();
        }

        UnorderedWhileSpliterator(T_SPLITR s, UnorderedWhileSpliterator<T, T_SPLITR> parent) {
            this.s = s;
            this.noSplitting = parent.noSplitting;
            this.cancel = parent.cancel;
        }

        @Override
        public long estimateSize() {
            return s.estimateSize();
        }

        @Override
        public int characteristics() {
            // Size is not known
            return s.characteristics() & ~(Spliterator.SIZED | Spliterator.SUBSIZED);
        }

        @Override
        public long getExactSizeIfKnown() {
            return -1L;
        }

        @Override
        public Comparator<? super T> getComparator() {
            return s.getComparator();
        }

        @Override
        public T_SPLITR trySplit() {
            @SuppressWarnings("unchecked")
            T_SPLITR ls = noSplitting ? null : (T_SPLITR) s.trySplit();
            return ls != null ? makeSpliterator(ls) : null;
        }

        boolean checkCancelOnCount() {
            return count != 0 || !cancel.get();
        }

        abstract T_SPLITR makeSpliterator(T_SPLITR s);

        abstract static class OfRef<T> extends UnorderedWhileSpliterator<T, Spliterator<T>> implements Consumer<T> {
            final Predicate<? super T> p;
            T t;

            OfRef(Spliterator<T> s, boolean noSplitting, Predicate<? super T> p) {
                super(s, noSplitting);
                this.p = p;
            }

            OfRef(Spliterator<T> s, OfRef<T> parent) {
                super(s, parent);
                this.p = parent.p;
            }

            @Override
            public void accept(T t) {
                count = (count + 1) & CANCEL_CHECK_COUNT;
                this.t = t;
            }

            static final class Taking<T> extends OfRef<T> {
                Taking(Spliterator<T> s, boolean noSplitting, Predicate<? super T> p) {
                    super(s, noSplitting, p);
                }

                Taking(Spliterator<T> s, Taking<T> parent) {
                    super(s, parent);
                }

                @Override
                public boolean tryAdvance(Consumer<? super T> action) {
                    boolean test = true;
                    if (takeOrDrop &&               // If can take
                        checkCancelOnCount() && // and if not cancelled
                        s.tryAdvance(this) &&   // and if advanced one element
                        (test = p.test(t))) {   // and test on element passes
                        action.accept(t);           // then accept element
                        return true;
                    }
                    else {
                        // Taking is finished
                        takeOrDrop = false;
                        // Cancel all further traversal and splitting operations
                        // only if test of element failed (short-circuited)
                        if (!test)
                            cancel.set(true);
                        return false;
                    }
                }

                @Override
                public Spliterator<T> trySplit() {
                    // Do not split if all operations are cancelled
                    return cancel.get() ? null : super.trySplit();
                }

                @Override
                Spliterator<T> makeSpliterator(Spliterator<T> s) {
                    return new Taking<>(s, this);
                }
            }

            static final class Dropping<T> extends OfRef<T> {
                Dropping(Spliterator<T> s, boolean noSplitting, Predicate<? super T> p) {
                    super(s, noSplitting, p);
                }

                Dropping(Spliterator<T> s, Dropping<T> parent) {
                    super(s, parent);
                }

                @Override
                public boolean tryAdvance(Consumer<? super T> action) {
                    if (takeOrDrop) {
                        takeOrDrop = false;
                        boolean adv;
                        boolean dropped = false;
                        while ((adv = s.tryAdvance(this)) &&  // If advanced one element
                               checkCancelOnCount() &&        // and if not cancelled
                               p.test(t)) {                   // and test on element passes
                            dropped = true;                   // then drop element
                        }

                        // Report advanced element, if any
                        if (adv) {
                            // Cancel all further dropping if one or more elements
                            // were previously dropped
                            if (dropped)
                                cancel.set(true);
                            action.accept(t);
                        }
                        return adv;
                    }
                    else {
                        return s.tryAdvance(action);
                    }
                }

                @Override
                Spliterator<T> makeSpliterator(Spliterator<T> s) {
                    return new Dropping<>(s, this);
                }
            }
        }

        abstract static class OfInt extends UnorderedWhileSpliterator<Integer, Spliterator.OfInt> implements IntConsumer, Spliterator.OfInt {
            final IntPredicate p;
            int t;

            OfInt(Spliterator.OfInt s, boolean noSplitting, IntPredicate p) {
                super(s, noSplitting);
                this.p = p;
            }

            OfInt(Spliterator.OfInt s, UnorderedWhileSpliterator.OfInt parent) {
                super(s, parent);
                this.p = parent.p;
            }

            @Override
            public void accept(int t) {
                count = (count + 1) & CANCEL_CHECK_COUNT;
                this.t = t;
            }

            static final class Taking extends UnorderedWhileSpliterator.OfInt {
                Taking(Spliterator.OfInt s, boolean noSplitting, IntPredicate p) {
                    super(s, noSplitting, p);
                }

                Taking(Spliterator.OfInt s, UnorderedWhileSpliterator.OfInt parent) {
                    super(s, parent);
                }

                @Override
                public boolean tryAdvance(IntConsumer action) {
                    boolean test = true;
                    if (takeOrDrop &&               // If can take
                        checkCancelOnCount() && // and if not cancelled
                        s.tryAdvance(this) &&   // and if advanced one element
                        (test = p.test(t))) {   // and test on element passes
                        action.accept(t);           // then accept element
                        return true;
                    }
                    else {
                        // Taking is finished
                        takeOrDrop = false;
                        // Cancel all further traversal and splitting operations
                        // only if test of element failed (short-circuited)
                        if (!test)
                            cancel.set(true);
                        return false;
                    }
                }

                @Override
                public Spliterator.OfInt trySplit() {
                    // Do not split if all operations are cancelled
                    return cancel.get() ? null : super.trySplit();
                }

                @Override
                Spliterator.OfInt makeSpliterator(Spliterator.OfInt s) {
                    return new Taking(s, this);
                }
            }

            static final class Dropping extends UnorderedWhileSpliterator.OfInt {
                Dropping(Spliterator.OfInt s, boolean noSplitting, IntPredicate p) {
                    super(s, noSplitting, p);
                }

                Dropping(Spliterator.OfInt s, UnorderedWhileSpliterator.OfInt parent) {
                    super(s, parent);
                }

                @Override
                public boolean tryAdvance(IntConsumer action) {
                    if (takeOrDrop) {
                        takeOrDrop = false;
                        boolean adv;
                        boolean dropped = false;
                        while ((adv = s.tryAdvance(this)) &&  // If advanced one element
                               checkCancelOnCount() &&        // and if not cancelled
                               p.test(t)) {                   // and test on element passes
                            dropped = true;                   // then drop element
                        }

                        // Report advanced element, if any
                        if (adv) {
                            // Cancel all further dropping if one or more elements
                            // were previously dropped
                            if (dropped)
                                cancel.set(true);
                            action.accept(t);
                        }
                        return adv;
                    }
                    else {
                        return s.tryAdvance(action);
                    }
                }

                @Override
                Spliterator.OfInt makeSpliterator(Spliterator.OfInt s) {
                    return new Dropping(s, this);
                }
            }
        }

        abstract static class OfLong extends UnorderedWhileSpliterator<Long, Spliterator.OfLong> implements LongConsumer, Spliterator.OfLong {
            final LongPredicate p;
            long t;

            OfLong(Spliterator.OfLong s, boolean noSplitting, LongPredicate p) {
                super(s, noSplitting);
                this.p = p;
            }

            OfLong(Spliterator.OfLong s, UnorderedWhileSpliterator.OfLong parent) {
                super(s, parent);
                this.p = parent.p;
            }

            @Override
            public void accept(long t) {
                count = (count + 1) & CANCEL_CHECK_COUNT;
                this.t = t;
            }

            static final class Taking extends UnorderedWhileSpliterator.OfLong {
                Taking(Spliterator.OfLong s, boolean noSplitting, LongPredicate p) {
                    super(s, noSplitting, p);
                }

                Taking(Spliterator.OfLong s, UnorderedWhileSpliterator.OfLong parent) {
                    super(s, parent);
                }

                @Override
                public boolean tryAdvance(LongConsumer action) {
                    boolean test = true;
                    if (takeOrDrop &&               // If can take
                        checkCancelOnCount() && // and if not cancelled
                        s.tryAdvance(this) &&   // and if advanced one element
                        (test = p.test(t))) {   // and test on element passes
                        action.accept(t);           // then accept element
                        return true;
                    }
                    else {
                        // Taking is finished
                        takeOrDrop = false;
                        // Cancel all further traversal and splitting operations
                        // only if test of element failed (short-circuited)
                        if (!test)
                            cancel.set(true);
                        return false;
                    }
                }

                @Override
                public Spliterator.OfLong trySplit() {
                    // Do not split if all operations are cancelled
                    return cancel.get() ? null : super.trySplit();
                }

                @Override
                Spliterator.OfLong makeSpliterator(Spliterator.OfLong s) {
                    return new Taking(s, this);
                }
            }

            static final class Dropping extends UnorderedWhileSpliterator.OfLong {
                Dropping(Spliterator.OfLong s, boolean noSplitting, LongPredicate p) {
                    super(s, noSplitting, p);
                }

                Dropping(Spliterator.OfLong s, UnorderedWhileSpliterator.OfLong parent) {
                    super(s, parent);
                }

                @Override
                public boolean tryAdvance(LongConsumer action) {
                    if (takeOrDrop) {
                        takeOrDrop = false;
                        boolean adv;
                        boolean dropped = false;
                        while ((adv = s.tryAdvance(this)) &&  // If advanced one element
                               checkCancelOnCount() &&        // and if not cancelled
                               p.test(t)) {                   // and test on element passes
                            dropped = true;                   // then drop element
                        }

                        // Report advanced element, if any
                        if (adv) {
                            // Cancel all further dropping if one or more elements
                            // were previously dropped
                            if (dropped)
                                cancel.set(true);
                            action.accept(t);
                        }
                        return adv;
                    }
                    else {
                        return s.tryAdvance(action);
                    }
                }

                @Override
                Spliterator.OfLong makeSpliterator(Spliterator.OfLong s) {
                    return new Dropping(s, this);
                }
            }
        }

        abstract static class OfDouble extends UnorderedWhileSpliterator<Double, Spliterator.OfDouble> implements DoubleConsumer, Spliterator.OfDouble {
            final DoublePredicate p;
            double t;

            OfDouble(Spliterator.OfDouble s, boolean noSplitting, DoublePredicate p) {
                super(s, noSplitting);
                this.p = p;
            }

            OfDouble(Spliterator.OfDouble s, UnorderedWhileSpliterator.OfDouble parent) {
                super(s, parent);
                this.p = parent.p;
            }

            @Override
            public void accept(double t) {
                count = (count + 1) & CANCEL_CHECK_COUNT;
                this.t = t;
            }

            static final class Taking extends UnorderedWhileSpliterator.OfDouble {
                Taking(Spliterator.OfDouble s, boolean noSplitting, DoublePredicate p) {
                    super(s, noSplitting, p);
                }

                Taking(Spliterator.OfDouble s, UnorderedWhileSpliterator.OfDouble parent) {
                    super(s, parent);
                }

                @Override
                public boolean tryAdvance(DoubleConsumer action) {
                    boolean test = true;
                    if (takeOrDrop &&               // If can take
                        checkCancelOnCount() && // and if not cancelled
                        s.tryAdvance(this) &&   // and if advanced one element
                        (test = p.test(t))) {   // and test on element passes
                        action.accept(t);           // then accept element
                        return true;
                    }
                    else {
                        // Taking is finished
                        takeOrDrop = false;
                        // Cancel all further traversal and splitting operations
                        // only if test of element failed (short-circuited)
                        if (!test)
                            cancel.set(true);
                        return false;
                    }
                }

                @Override
                public Spliterator.OfDouble trySplit() {
                    // Do not split if all operations are cancelled
                    return cancel.get() ? null : super.trySplit();
                }

                @Override
                Spliterator.OfDouble makeSpliterator(Spliterator.OfDouble s) {
                    return new Taking(s, this);
                }
            }

            static final class Dropping extends UnorderedWhileSpliterator.OfDouble {
                Dropping(Spliterator.OfDouble s, boolean noSplitting, DoublePredicate p) {
                    super(s, noSplitting, p);
                }

                Dropping(Spliterator.OfDouble s, UnorderedWhileSpliterator.OfDouble parent) {
                    super(s, parent);
                }

                @Override
                public boolean tryAdvance(DoubleConsumer action) {
                    if (takeOrDrop) {
                        takeOrDrop = false;
                        boolean adv;
                        boolean dropped = false;
                        while ((adv = s.tryAdvance(this)) &&  // If advanced one element
                               checkCancelOnCount() &&        // and if not cancelled
                               p.test(t)) {                   // and test on element passes
                            dropped = true;                   // then drop element
                        }

                        // Report advanced element, if any
                        if (adv) {
                            // Cancel all further dropping if one or more elements
                            // were previously dropped
                            if (dropped)
                                cancel.set(true);
                            action.accept(t);
                        }
                        return adv;
                    }
                    else {
                        return s.tryAdvance(action);
                    }
                }

                @Override
                Spliterator.OfDouble makeSpliterator(Spliterator.OfDouble s) {
                    return new Dropping(s, this);
                }
            }
        }
    }
}
