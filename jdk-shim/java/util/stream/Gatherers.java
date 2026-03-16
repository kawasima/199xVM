/*
 * Copyright (c) 2023, 2025, Oracle and/or its affiliates. All rights reserved.
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
import java.util.function.BiConsumer;
import java.util.function.BinaryOperator;
import java.util.function.Supplier;
import java.util.stream.Gatherer.Integrator;
import java.util.stream.Gatherer.Downstream;

/**
 * Implementations of {@link Gatherer} that provide useful intermediate
 * operations, such as windowing functions, folding functions,
 * transforming elements concurrently, etc.
 *
 * <p>This is a minimal shim providing only the internal types needed by
 * {@link Gatherer} -- {@code Value}, {@code GathererImpl}, and
 * {@code Composite}.
 *
 * @since 24
 */
public final class Gatherers {
    private Gatherers() { } // This class is not intended to be instantiated

    /**
     * Default value singleton for stateless/sequential/no-finisher gatherers.
     */
    @SuppressWarnings("rawtypes")
    enum Value implements Supplier, BinaryOperator, BiConsumer {
        DEFAULT;

        final BinaryOperator<Void> statelessCombiner = new BinaryOperator<>() {
            @Override public Void apply(Void left, Void right) { return null; }
        };

        // BiConsumer
        @Override public void accept(Object state, Object downstream) {}

        // BinaryOperator
        @Override public Object apply(Object left, Object right) {
            throw new UnsupportedOperationException("This combiner cannot be used!");
        }

        // Supplier
        @Override public Object get() { return null; }

        @SuppressWarnings("unchecked")
        <A> Supplier<A> initializer() { return (Supplier<A>)this; }

        @SuppressWarnings("unchecked")
        <T> BinaryOperator<T> combiner() { return (BinaryOperator<T>) this; }

        @SuppressWarnings("unchecked")
        <T, R> BiConsumer<T, Gatherer.Downstream<? super R>> finisher() {
            return (BiConsumer<T, Downstream<? super R>>) this;
        }
    }

    record GathererImpl<T, A, R>(
            @Override Supplier<A> initializer,
            @Override Integrator<A, T, R> integrator,
            @Override BinaryOperator<A> combiner,
            @Override BiConsumer<A, Downstream<? super R>> finisher) implements Gatherer<T, A, R> {

        static <T, A, R> GathererImpl<T, A, R> of(
                Supplier<A> initializer,
                Integrator<A, T, R> integrator,
                BinaryOperator<A> combiner,
                BiConsumer<A, Downstream<? super R>> finisher) {
            return new GathererImpl<>(
                    Objects.requireNonNull(initializer,"initializer"),
                    Objects.requireNonNull(integrator, "integrator"),
                    Objects.requireNonNull(combiner, "combiner"),
                    Objects.requireNonNull(finisher, "finisher")
            );
        }
    }

    static final class Composite<T, A, R, AA, RR> implements Gatherer<T, Object, RR> {
        private final Gatherer<T, A, ? extends R> left;
        private final Gatherer<? super R, AA, ? extends RR> right;
        private GathererImpl<T, Object, RR> impl;

        static <T, A, R, AA, RR> Composite<T, A, R, AA, RR> of(
                Gatherer<T, A, ? extends R> left,
                Gatherer<? super R, AA, ? extends RR> right) {
            return new Composite<>(left, right);
        }

        private Composite(Gatherer<T, A, ? extends R> left,
                          Gatherer<? super R, AA, ? extends RR> right) {
            this.left = left;
            this.right = right;
        }

        @SuppressWarnings("unchecked")
        private GathererImpl<T, Object, RR> impl() {
            var i = impl;
            return i != null
                     ? i
                     : (impl = (GathererImpl<T, Object, RR>)impl(left, right));
        }

        @Override public Supplier<Object> initializer() {
            return impl().initializer();
        }

        @Override public Integrator<Object, T, RR> integrator() {
            return impl().integrator();
        }

        @Override public BinaryOperator<Object> combiner() {
            return impl().combiner();
        }

        @Override public BiConsumer<Object, Downstream<? super RR>> finisher() {
            return impl().finisher();
        }

        @Override
        public <RRR> Gatherer<T, ?, RRR> andThen(
                Gatherer<? super RR, ?, ? extends RRR> that) {
            if (that.getClass() == Composite.class) {
                @SuppressWarnings("unchecked")
                final var c =
                    (Composite<? super RR, ?, Object, ?, ? extends RRR>) that;
                return left.andThen(right.andThen(c.left).andThen(c.right));
            } else {
                return left.andThen(right.andThen(that));
            }
        }

        static final <T, A, R, AA, RR> GathererImpl<T, ?, RR> impl(
                Gatherer<T, A, R> left, Gatherer<? super R, AA, RR> right) {
            final var leftInitializer = left.initializer();
            final var leftIntegrator = left.integrator();
            final var leftCombiner = left.combiner();
            final var leftFinisher = left.finisher();

            final var rightInitializer = right.initializer();
            final var rightIntegrator = right.integrator();
            final var rightCombiner = right.combiner();
            final var rightFinisher = right.finisher();

            final var leftStateless = leftInitializer == Gatherer.defaultInitializer();
            final var rightStateless = rightInitializer == Gatherer.defaultInitializer();

            final var leftGreedy = leftIntegrator instanceof Integrator.Greedy;
            final var rightGreedy = rightIntegrator instanceof Integrator.Greedy;

            /*
             * For pairs of stateless and greedy Gatherers, we can optimize
             * evaluation as we do not need to track any state nor any
             * short-circuit signals.
             */
            if (leftStateless && rightStateless && leftGreedy && rightGreedy) {
                return new GathererImpl<>(
                    Gatherer.defaultInitializer(),
                    Gatherer.Integrator.ofGreedy((unused, element, downstream) ->
                        leftIntegrator.integrate(
                                null,
                                element,
                                r -> rightIntegrator.integrate(null, r, downstream))
                    ),
                    (leftCombiner == Gatherer.defaultCombiner()
                    || rightCombiner == Gatherer.defaultCombiner())
                            ? Gatherer.defaultCombiner()
                            : Value.DEFAULT.statelessCombiner
                    ,
                    (leftFinisher == Gatherer.<A,R>defaultFinisher()
                    && rightFinisher == Gatherer.<AA,RR>defaultFinisher())
                            ? Gatherer.defaultFinisher()
                            : (unused, downstream) -> {
                        if (leftFinisher != Gatherer.<A,R>defaultFinisher())
                            leftFinisher.accept(
                                    null,
                                    r -> rightIntegrator.integrate(null, r, downstream));
                        if (rightFinisher != Gatherer.<AA,RR>defaultFinisher())
                            rightFinisher.accept(null, downstream);
                    }
                );
            } else {
                class State {
                    final A leftState;
                    final AA rightState;
                    boolean leftProceed;
                    boolean rightProceed;

                    private State(A leftState, AA rightState,
                                  boolean leftProceed, boolean rightProceed) {
                        this.leftState = leftState;
                        this.rightState = rightState;
                        this.leftProceed = leftProceed;
                        this.rightProceed = rightProceed;
                    }

                    State() {
                        this(leftStateless ? null : leftInitializer.get(),
                             rightStateless ? null : rightInitializer.get(),
                            true, true);
                    }

                    State joinLeft(State right) {
                        return new State(
                                leftStateless ? null : leftCombiner.apply(this.leftState, right.leftState),
                                rightStateless ? null : rightCombiner.apply(this.rightState, right.rightState),
                                this.leftProceed && this.rightProceed,
                                right.leftProceed && right.rightProceed);
                    }

                    boolean integrate(T t, Downstream<? super RR> c) {
                        return (leftIntegrator.integrate(leftState, t, r -> rightIntegrate(r, c))
                                  || leftGreedy
                                  || (leftProceed = false))
                                && (rightGreedy || rightProceed);
                    }

                    void finish(Downstream<? super RR> c) {
                        if (leftFinisher != Gatherer.<A, R>defaultFinisher())
                            leftFinisher.accept(leftState, r -> rightIntegrate(r, c));
                        if (rightFinisher != Gatherer.<AA, RR>defaultFinisher())
                            rightFinisher.accept(rightState, c);
                    }

                    public boolean rightIntegrate(R r, Downstream<? super RR> downstream) {
                        return (rightGreedy || rightProceed)
                                && (rightIntegrator.integrate(rightState, r, downstream)
                                || rightGreedy
                                || (rightProceed = false));
                    }
                }

                return new GathererImpl<T, State, RR>(
                        State::new,
                        (leftGreedy && rightGreedy)
                                ? Integrator.<State, T, RR>ofGreedy(State::integrate)
                                : Integrator.<State, T, RR>of(State::integrate),
                        (leftCombiner == Gatherer.defaultCombiner()
                        || rightCombiner == Gatherer.defaultCombiner())
                                ? Gatherer.defaultCombiner()
                                : State::joinLeft,
                        (leftFinisher == Gatherer.<A, R>defaultFinisher()
                        && rightFinisher == Gatherer.<AA, RR>defaultFinisher())
                                ? Gatherer.defaultFinisher()
                                : State::finish
                );
            }
        }
    }
}
