package java.util.concurrent;

import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Supplier;

public class CompletableFuture<T> implements Future<T>, CompletionStage<T> {
    private T result;
    private Throwable exception;
    private boolean done;
    private boolean cancelled;

    public CompletableFuture() {}

    private static <U> CompletableFuture<U> failed(Throwable t) {
        CompletableFuture<U> cf = new CompletableFuture<>();
        cf.completeExceptionally(t);
        return cf;
    }

    private static Executor syncExecutor() {
        return r -> r.run();
    }

    private static <U> CompletableFuture<U> fromStage(CompletionStage<? extends U> stage) {
        if (stage instanceof CompletableFuture<?> cf) {
            @SuppressWarnings("unchecked")
            CompletableFuture<U> out = (CompletableFuture<U>) cf;
            return out;
        }
        @SuppressWarnings("unchecked")
        CompletableFuture<U> out = (CompletableFuture<U>) stage.toCompletableFuture();
        return out;
    }

    public static <U> CompletableFuture<U> supplyAsync(Supplier<U> supplier) {
        return supplyAsync(supplier, syncExecutor());
    }

    public static <U> CompletableFuture<U> supplyAsync(Supplier<U> supplier, Executor executor) {
        CompletableFuture<U> cf = new CompletableFuture<>();
        executor.execute(() -> {
            try {
                cf.complete(supplier.get());
            } catch (Throwable t) {
                cf.completeExceptionally(t);
            }
        });
        return cf;
    }

    public static CompletableFuture<Void> runAsync(Runnable runnable) {
        return runAsync(runnable, syncExecutor());
    }

    public static CompletableFuture<Void> runAsync(Runnable runnable, Executor executor) {
        CompletableFuture<Void> cf = new CompletableFuture<>();
        executor.execute(() -> {
            try {
                runnable.run();
                cf.complete(null);
            } catch (Throwable t) {
                cf.completeExceptionally(t);
            }
        });
        return cf;
    }

    public static <U> CompletableFuture<U> completedFuture(U value) {
        CompletableFuture<U> cf = new CompletableFuture<>();
        cf.complete(value);
        return cf;
    }

    public boolean isDone() { return done; }

    public T get() throws InterruptedException, ExecutionException {
        if (!done) {
            throw new InterruptedException();
        }
        if (cancelled) throw new CancellationException();
        if (exception != null) throw new ExecutionException(exception);
        return result;
    }

    public T get(long timeout, TimeUnit unit) throws InterruptedException, ExecutionException, TimeoutException {
        return get();
    }

    public T join() {
        if (!done) throw new CompletionException(new IllegalStateException("Not completed"));
        if (cancelled) throw new CancellationException();
        if (exception != null) throw new CompletionException(exception);
        return result;
    }

    public T getNow(T valueIfAbsent) {
        if (!done || cancelled || exception != null) return valueIfAbsent;
        return result;
    }

    public T resultNow() {
        if (state() != Future.State.SUCCESS) throw new IllegalStateException();
        return result;
    }

    public Throwable exceptionNow() {
        if (state() != Future.State.FAILED) throw new IllegalStateException();
        return exception;
    }

    public boolean complete(T value) {
        if (done) return false;
        result = value;
        done = true;
        return true;
    }

    public boolean completeExceptionally(Throwable ex) {
        if (done) return false;
        exception = ex;
        done = true;
        return true;
    }

    public <U> CompletableFuture<U> thenApply(Function<? super T, ? extends U> fn) {
        if (exception != null) return failed(exception);
        if (cancelled) return failed(new CancellationException());
        try {
            return completedFuture(fn.apply(join()));
        } catch (Throwable t) {
            return failed(t);
        }
    }

    public <U> CompletableFuture<U> thenApplyAsync(Function<? super T, ? extends U> fn) {
        return thenApplyAsync(fn, defaultExecutor());
    }

    public <U> CompletableFuture<U> thenApplyAsync(Function<? super T, ? extends U> fn, Executor executor) {
        return supplyAsync(() -> fn.apply(join()), executor);
    }

    public CompletableFuture<Void> thenAccept(Consumer<? super T> action) {
        return thenApply(v -> {
            action.accept(v);
            return null;
        });
    }

    public CompletableFuture<Void> thenAcceptAsync(Consumer<? super T> action) {
        return thenAcceptAsync(action, defaultExecutor());
    }

    public CompletableFuture<Void> thenAcceptAsync(Consumer<? super T> action, Executor executor) {
        return thenApplyAsync(v -> {
            action.accept(v);
            return null;
        }, executor);
    }

    public CompletableFuture<Void> thenRun(Runnable action) {
        return thenApply(v -> {
            action.run();
            return null;
        });
    }

    public CompletableFuture<Void> thenRunAsync(Runnable action) {
        return thenRunAsync(action, defaultExecutor());
    }

    public CompletableFuture<Void> thenRunAsync(Runnable action, Executor executor) {
        return thenApplyAsync(v -> {
            action.run();
            return null;
        }, executor);
    }

    public <U, V> CompletableFuture<V> thenCombine(CompletionStage<? extends U> other,
                                                    BiFunction<? super T, ? super U, ? extends V> fn) {
        try {
            U u = fromStage(other).join();
            return completedFuture(fn.apply(join(), u));
        } catch (Throwable t) {
            return failed(t);
        }
    }

    public <U, V> CompletableFuture<V> thenCombineAsync(CompletionStage<? extends U> other,
                                                         BiFunction<? super T, ? super U, ? extends V> fn) {
        return thenCombineAsync(other, fn, defaultExecutor());
    }

    public <U, V> CompletableFuture<V> thenCombineAsync(CompletionStage<? extends U> other,
                                                         BiFunction<? super T, ? super U, ? extends V> fn,
                                                         Executor executor) {
        return supplyAsync(() -> fn.apply(join(), fromStage(other).join()), executor);
    }

    public <U> CompletableFuture<Void> thenAcceptBoth(CompletionStage<? extends U> other,
                                                       BiConsumer<? super T, ? super U> action) {
        return thenCombine(other, (a, b) -> {
            action.accept(a, b);
            return null;
        });
    }

    public <U> CompletableFuture<Void> thenAcceptBothAsync(CompletionStage<? extends U> other,
                                                            BiConsumer<? super T, ? super U> action) {
        return thenAcceptBothAsync(other, action, defaultExecutor());
    }

    public <U> CompletableFuture<Void> thenAcceptBothAsync(CompletionStage<? extends U> other,
                                                            BiConsumer<? super T, ? super U> action,
                                                            Executor executor) {
        return thenCombineAsync(other, (a, b) -> {
            action.accept(a, b);
            return null;
        }, executor);
    }

    public CompletableFuture<Void> runAfterBoth(CompletionStage<?> other, Runnable action) {
        return thenCombine(other, (a, b) -> {
            action.run();
            return null;
        });
    }

    public CompletableFuture<Void> runAfterBothAsync(CompletionStage<?> other, Runnable action) {
        return runAfterBothAsync(other, action, defaultExecutor());
    }

    public CompletableFuture<Void> runAfterBothAsync(CompletionStage<?> other, Runnable action, Executor executor) {
        return thenCombineAsync(other, (a, b) -> {
            action.run();
            return null;
        }, executor);
    }

    public <U> CompletableFuture<U> applyToEither(CompletionStage<? extends T> other,
                                                   Function<? super T, U> fn) {
        try {
            return completedFuture(fn.apply(join()));
        } catch (Throwable t) {
            try {
                return completedFuture(fn.apply(fromStage(other).join()));
            } catch (Throwable t2) {
                return failed(t2);
            }
        }
    }

    public <U> CompletableFuture<U> applyToEitherAsync(CompletionStage<? extends T> other,
                                                        Function<? super T, U> fn) {
        return applyToEitherAsync(other, fn, defaultExecutor());
    }

    public <U> CompletableFuture<U> applyToEitherAsync(CompletionStage<? extends T> other,
                                                        Function<? super T, U> fn,
                                                        Executor executor) {
        return supplyAsync(() -> applyToEither(other, fn).join(), executor);
    }

    public CompletableFuture<Void> acceptEither(CompletionStage<? extends T> other,
                                                Consumer<? super T> action) {
        return applyToEither(other, v -> {
            action.accept(v);
            return null;
        });
    }

    public CompletableFuture<Void> acceptEitherAsync(CompletionStage<? extends T> other,
                                                     Consumer<? super T> action) {
        return acceptEitherAsync(other, action, defaultExecutor());
    }

    public CompletableFuture<Void> acceptEitherAsync(CompletionStage<? extends T> other,
                                                     Consumer<? super T> action,
                                                     Executor executor) {
        return applyToEitherAsync(other, v -> {
            action.accept(v);
            return null;
        }, executor);
    }

    public CompletableFuture<Void> runAfterEither(CompletionStage<?> other, Runnable action) {
        return applyToEither((CompletionStage<? extends T>) other, v -> {
            action.run();
            return null;
        });
    }

    public CompletableFuture<Void> runAfterEitherAsync(CompletionStage<?> other, Runnable action) {
        return runAfterEitherAsync(other, action, defaultExecutor());
    }

    public CompletableFuture<Void> runAfterEitherAsync(CompletionStage<?> other, Runnable action, Executor executor) {
        return applyToEitherAsync((CompletionStage<? extends T>) other, v -> {
            action.run();
            return null;
        }, executor);
    }

    public <U> CompletableFuture<U> thenCompose(Function<? super T, ? extends CompletionStage<U>> fn) {
        try {
            return fromStage(fn.apply(join()));
        } catch (Throwable t) {
            return failed(t);
        }
    }

    public <U> CompletableFuture<U> thenComposeAsync(Function<? super T, ? extends CompletionStage<U>> fn) {
        return thenComposeAsync(fn, defaultExecutor());
    }

    public <U> CompletableFuture<U> thenComposeAsync(Function<? super T, ? extends CompletionStage<U>> fn,
                                                     Executor executor) {
        return supplyAsync(() -> thenCompose(fn).join(), executor);
    }

    public CompletableFuture<T> whenComplete(BiConsumer<? super T, ? super Throwable> action) {
        action.accept(done ? result : null, done ? exception : null);
        return this;
    }

    public CompletableFuture<T> whenCompleteAsync(BiConsumer<? super T, ? super Throwable> action) {
        return whenCompleteAsync(action, defaultExecutor());
    }

    public CompletableFuture<T> whenCompleteAsync(BiConsumer<? super T, ? super Throwable> action, Executor executor) {
        executor.execute(() -> action.accept(done ? result : null, done ? exception : null));
        return this;
    }

    public <U> CompletableFuture<U> handle(BiFunction<? super T, Throwable, ? extends U> fn) {
        try {
            return completedFuture(fn.apply(done ? result : null, done ? exception : null));
        } catch (Throwable t) {
            return failed(t);
        }
    }

    public <U> CompletableFuture<U> handleAsync(BiFunction<? super T, Throwable, ? extends U> fn) {
        return handleAsync(fn, defaultExecutor());
    }

    public <U> CompletableFuture<U> handleAsync(BiFunction<? super T, Throwable, ? extends U> fn, Executor executor) {
        return supplyAsync(() -> fn.apply(done ? result : null, done ? exception : null), executor);
    }

    public CompletableFuture<T> toCompletableFuture() {
        return this;
    }

    public CompletableFuture<T> exceptionally(Function<Throwable, ? extends T> fn) {
        if (exception == null) return this;
        return completedFuture(fn.apply(exception));
    }

    public CompletableFuture<T> exceptionallyAsync(Function<Throwable, ? extends T> fn) {
        return exceptionallyAsync(fn, defaultExecutor());
    }

    public CompletableFuture<T> exceptionallyAsync(Function<Throwable, ? extends T> fn, Executor executor) {
        if (exception == null) return this;
        return supplyAsync(() -> fn.apply(exception), executor);
    }

    public CompletableFuture<T> exceptionallyCompose(Function<Throwable, ? extends CompletionStage<T>> fn) {
        if (exception == null) return this;
        return fromStage(fn.apply(exception));
    }

    public CompletableFuture<T> exceptionallyComposeAsync(Function<Throwable, ? extends CompletionStage<T>> fn) {
        return exceptionallyComposeAsync(fn, defaultExecutor());
    }

    public CompletableFuture<T> exceptionallyComposeAsync(Function<Throwable, ? extends CompletionStage<T>> fn,
                                                          Executor executor) {
        if (exception == null) return this;
        return supplyAsync(() -> fromStage(fn.apply(exception)).join(), executor);
    }

    public static CompletableFuture<Void> allOf(CompletableFuture<?>... cfs) {
        for (CompletableFuture<?> cf : cfs) {
            cf.join();
        }
        return completedFuture(null);
    }

    public static CompletableFuture<Object> anyOf(CompletableFuture<?>... cfs) {
        if (cfs.length == 0) return completedFuture(null);
        return completedFuture(cfs[0].join());
    }

    public boolean cancel(boolean mayInterruptIfRunning) {
        if (done) return false;
        cancelled = true;
        done = true;
        return true;
    }

    public boolean isCancelled() { return cancelled; }

    public boolean isCompletedExceptionally() {
        return done && (exception != null || cancelled);
    }

    public Future.State state() {
        if (!done) return Future.State.RUNNING;
        if (cancelled) return Future.State.CANCELLED;
        if (exception != null) return Future.State.FAILED;
        return Future.State.SUCCESS;
    }

    public void obtrudeValue(T value) {
        this.result = value;
        this.exception = null;
        this.cancelled = false;
        this.done = true;
    }

    public void obtrudeException(Throwable ex) {
        this.exception = ex;
        this.done = true;
    }

    public int getNumberOfDependents() { return 0; }

    public String toString() {
        return "CompletableFuture[" + state() + "]";
    }

    public <U> CompletableFuture<U> newIncompleteFuture() {
        return new CompletableFuture<>();
    }

    public Executor defaultExecutor() {
        return syncExecutor();
    }

    public CompletableFuture<T> copy() {
        if (!done) return new CompletableFuture<>();
        if (cancelled) {
            CompletableFuture<T> out = new CompletableFuture<>();
            out.cancel(false);
            return out;
        }
        if (exception != null) return failed(exception);
        return completedFuture(result);
    }

    public CompletionStage<T> minimalCompletionStage() {
        return this;
    }

    public CompletableFuture<T> completeAsync(Supplier<? extends T> supplier, Executor executor) {
        executor.execute(() -> {
            try {
                complete(supplier.get());
            } catch (Throwable t) {
                completeExceptionally(t);
            }
        });
        return this;
    }

    public CompletableFuture<T> completeAsync(Supplier<? extends T> supplier) {
        return completeAsync(supplier, defaultExecutor());
    }

    public CompletableFuture<T> orTimeout(long timeout, TimeUnit unit) {
        return this;
    }

    public CompletableFuture<T> completeOnTimeout(T value, long timeout, TimeUnit unit) {
        if (!done) complete(value);
        return this;
    }

    public static Executor delayedExecutor(long delay, TimeUnit unit, Executor executor) {
        return executor;
    }

    public static Executor delayedExecutor(long delay, TimeUnit unit) {
        return syncExecutor();
    }

    public static <U> CompletionStage<U> completedStage(U value) {
        return completedFuture(value);
    }

    public static <U> CompletableFuture<U> failedFuture(Throwable ex) {
        return failed(ex);
    }

    public static <U> CompletionStage<U> failedStage(Throwable ex) {
        return failed(ex);
    }
}
