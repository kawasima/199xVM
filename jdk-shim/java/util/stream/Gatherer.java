/*
 * Copyright (c) 2023, 2024, Oracle and/or its affiliates. All rights reserved.
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

/**
 * An intermediate operation that transforms a stream of input elements into a
 * stream of output elements, optionally applying a final action when the end of
 * the upstream is reached.
 *
 * @param <T> the type of input elements to the gatherer operation
 * @param <A> the potentially mutable state type of the gatherer operation
 * @param <R> the type of output elements from the gatherer operation
 * @since 24
 */
public interface Gatherer<T, A, R> {
    /**
     * A function that produces an instance of the intermediate state used for
     * this gathering operation.
     *
     * @return A function that produces an instance of the intermediate state
     * used for this gathering operation
     */
    default Supplier<A> initializer() {
        return defaultInitializer();
    }

    /**
     * A function which integrates provided elements, potentially using
     * the provided intermediate state, optionally producing output to the
     * provided {@link Downstream}.
     *
     * @return a function which integrates provided elements
     */
    Integrator<A, T, R> integrator();

    /**
     * A function which accepts two intermediate states and combines them into
     * one.
     *
     * @return a function which accepts two intermediate states and combines
     *         them into one
     */
    default BinaryOperator<A> combiner() {
        return defaultCombiner();
    }

    /**
     * A function which accepts the final intermediate state
     * and a {@link Downstream} object, allowing to perform a final action at
     * the end of input elements.
     *
     * @return a function which transforms the intermediate result to the final
     *         result(s) which are then passed on to the provided Downstream
     */
    default BiConsumer<A, Downstream<? super R>> finisher() {
        return defaultFinisher();
    }

    /**
     * Returns a composed Gatherer which connects the output of this Gatherer
     * to the input of that Gatherer.
     *
     * @param that the other gatherer
     * @param <RR> The type of output of that Gatherer
     * @throws NullPointerException if the argument is {@code null}
     * @return returns a composed Gatherer which connects the output of this
     *         Gatherer as input that Gatherer
     */
    default <RR> Gatherer<T, ?, RR> andThen(Gatherer<? super R, ?, ? extends RR> that) {
        Objects.requireNonNull(that);
        return Gatherers.Composite.of(this, that);
    }

    /**
     * Returns an initializer which is the default initializer of a Gatherer.
     * The returned initializer identifies that the owner Gatherer is stateless.
     *
     * @return the instance of the default initializer
     * @param <A> the type of the state of the returned initializer
     */
    static <A> Supplier<A> defaultInitializer() {
        return Gatherers.Value.DEFAULT.initializer();
    }

    /**
     * Returns a combiner which is the default combiner of a Gatherer.
     * The returned combiner identifies that the owning Gatherer must only
     * be evaluated sequentially.
     *
     * @return the instance of the default combiner
     * @param <A> the type of the state of the returned combiner
     */
    static <A> BinaryOperator<A> defaultCombiner() {
        return Gatherers.Value.DEFAULT.combiner();
    }

    /**
     * Returns a {@code finisher} which is the default finisher of
     * a {@code Gatherer}.
     *
     * @return the instance of the default finisher
     * @param <A> the type of the state of the returned finisher
     * @param <R> the type of the Downstream of the returned finisher
     */
    static <A, R> BiConsumer<A, Downstream<? super R>> defaultFinisher() {
        return Gatherers.Value.DEFAULT.finisher();
    }

    /**
     * Returns a new, sequential, and stateless {@code Gatherer} described by
     * the given {@code integrator}.
     */
    static <T, R> Gatherer<T, Void, R> ofSequential(
            Integrator<Void, T, R> integrator) {
        return of(
                defaultInitializer(),
                integrator,
                defaultCombiner(),
                defaultFinisher()
        );
    }

    /**
     * Returns a new, sequential, and stateless {@code Gatherer} described by
     * the given {@code integrator} and {@code finisher}.
     */
    static <T, R> Gatherer<T, Void, R> ofSequential(
            Integrator<Void, T, R> integrator,
            BiConsumer<Void, Downstream<? super R>> finisher) {
        return of(
                defaultInitializer(),
                integrator,
                defaultCombiner(),
                finisher
        );
    }

    /**
     * Returns a new, sequential, {@code Gatherer} described by the given
     * {@code initializer} and {@code integrator}.
     */
    static <T, A, R> Gatherer<T, A, R> ofSequential(
            Supplier<A> initializer,
            Integrator<A, T, R> integrator) {
        return of(
                initializer,
                integrator,
                defaultCombiner(),
                defaultFinisher()
        );
    }

    /**
     * Returns a new, sequential, {@code Gatherer} described by the given
     * {@code initializer}, {@code integrator}, and {@code finisher}.
     */
    static <T, A, R> Gatherer<T, A, R> ofSequential(
            Supplier<A> initializer,
            Integrator<A, T, R> integrator,
            BiConsumer<A, Downstream<? super R>> finisher) {
        return of(
                initializer,
                integrator,
                defaultCombiner(),
                finisher
        );
    }

    /**
     * Returns a new, parallelizable, and stateless {@code Gatherer} described
     * by the given {@code integrator}.
     */
    static <T, R> Gatherer<T, Void, R> of(Integrator<Void, T, R> integrator) {
        return of(
                defaultInitializer(),
                integrator,
                Gatherers.Value.DEFAULT.statelessCombiner,
                defaultFinisher()
        );
    }

    /**
     * Returns a new, parallelizable, and stateless {@code Gatherer} described
     * by the given {@code integrator} and {@code finisher}.
     */
    static <T, R> Gatherer<T, Void, R> of(
            Integrator<Void, T, R> integrator,
            BiConsumer<Void, Downstream<? super R>> finisher) {
        return of(
                defaultInitializer(),
                integrator,
                Gatherers.Value.DEFAULT.statelessCombiner,
                finisher
        );
    }

    /**
     * Returns a new, parallelizable, {@code Gatherer} described by the given
     * {@code initializer}, {@code integrator}, {@code combiner} and
     * {@code finisher}.
     */
    static <T, A, R> Gatherer<T, A, R> of(
            Supplier<A> initializer,
            Integrator<A, T, R> integrator,
            BinaryOperator<A> combiner,
            BiConsumer<A, Downstream<? super R>> finisher) {
        return new Gatherers.GathererImpl<>(
                Objects.requireNonNull(initializer),
                Objects.requireNonNull(integrator),
                Objects.requireNonNull(combiner),
                Objects.requireNonNull(finisher)
        );
    }

    /**
     * A Downstream object is the next stage in a pipeline of operations,
     * to which elements can be sent.
     * @param <T> the type of elements this downstream accepts
     * @since 24
     */
    @FunctionalInterface
    interface Downstream<T> {

        /**
         * Pushes, if possible, the provided element downstream -- to the next
         * stage in the pipeline.
         *
         * @param element the element to push downstream
         * @return {@code true} if more elements can be sent,
         *         and {@code false} if not.
         */
        boolean push(T element);

        /**
         * Checks whether the next stage is known to not want
         * any more elements sent to it.
         *
         * @return {@code true} if this Downstream is known not to want any
         *         more elements sent to it, {@code false} if otherwise
         */
        default boolean isRejecting() { return false; }
    }

    /**
     * An Integrator receives elements and processes them,
     * optionally using the supplied state, and optionally sends incremental
     * results downstream.
     *
     * @param <A> the type of state used by this integrator
     * @param <T> the type of elements this integrator consumes
     * @param <R> the type of results this integrator can produce
     * @since 24
     */
    @FunctionalInterface
    interface Integrator<A, T, R> {
        /**
         * Performs an action given: the current state, the next element, and
         * a downstream object; potentially inspecting and/or updating
         * the state, optionally sending any number of elements downstream
         * -- and then returns whether more elements are to be consumed or not.
         *
         * @param state The state to integrate into
         * @param element The element to integrate
         * @param downstream The downstream object of this integration
         * @return {@code true} if subsequent integration is desired,
         *         {@code false} if not
         */
        boolean integrate(A state, T element, Downstream<? super R> downstream);

        /**
         * Factory method for turning Integrator-shaped lambdas into
         * Integrators.
         */
        static <A, T, R> Integrator<A, T, R> of(Integrator<A, T, R> integrator) {
            return integrator;
        }

        /**
         * Factory method for turning Integrator-shaped lambdas into
         * {@link Greedy} Integrators.
         */
        static <A, T, R> Greedy<A, T, R> ofGreedy(Greedy<A, T, R> greedy) {
            return greedy;
        }

        /**
         * Greedy Integrators consume all their input, and may only relay that
         * the downstream does not want more elements.
         *
         * @param <A> the type of state used by this integrator
         * @param <T> the type of elements this greedy integrator receives
         * @param <R> the type of results this greedy integrator can produce
         * @since 24
         */
        @FunctionalInterface
        interface Greedy<A, T, R> extends Integrator<A, T, R> { }
    }
}
