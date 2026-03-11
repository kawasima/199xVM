package java.util.concurrent;

import java.lang.Thread.UncaughtExceptionHandler;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Predicate;

public class ForkJoinPool extends AbstractExecutorService implements ScheduledExecutorService {
    public static interface ManagedBlocker {
        boolean block() throws InterruptedException;
        boolean isReleasable();
    }

    public static interface ForkJoinWorkerThreadFactory {
        ForkJoinWorkerThread newThread(ForkJoinPool pool);
    }

    public static final ForkJoinWorkerThreadFactory defaultForkJoinWorkerThreadFactory =
        new ForkJoinWorkerThreadFactory() {
            public ForkJoinWorkerThread newThread(ForkJoinPool pool) {
                return new ForkJoinWorkerThread(pool);
            }
        };

    private static final ForkJoinPool COMMON = new ForkJoinPool();

    public ForkJoinPool() {}
    public ForkJoinPool(int parallelism) {}
    public ForkJoinPool(int parallelism, ForkJoinWorkerThreadFactory factory,
                        UncaughtExceptionHandler handler, boolean asyncMode) {}
    public ForkJoinPool(int parallelism, ForkJoinWorkerThreadFactory factory,
                        UncaughtExceptionHandler handler, boolean asyncMode,
                        int corePoolSize, int maximumPoolSize, int minimumRunnable,
                        Predicate<? super ForkJoinPool> saturate,
                        long keepAliveTime, TimeUnit unit) {}

    public static ForkJoinPool commonPool() { return COMMON; }

    public <T> T invoke(ForkJoinTask<T> task) { return task.invoke(); }

    public void execute(ForkJoinTask<?> task) { task.fork(); }

    public void execute(Runnable task) {
        submit(task);
    }

    public <T> ForkJoinTask<T> submit(ForkJoinTask<T> task) {
        task.fork();
        return task;
    }

    public <T> ForkJoinTask<T> submit(Callable<T> task) {
        ForkJoinTask<T> f = ForkJoinTask.adapt(task);
        f.fork();
        return f;
    }

    public <T> ForkJoinTask<T> submit(Runnable task, T result) {
        ForkJoinTask<T> f = ForkJoinTask.adapt(task, result);
        f.fork();
        return f;
    }

    public ForkJoinTask<?> submit(Runnable task) {
        ForkJoinTask<?> f = ForkJoinTask.adapt(task);
        f.fork();
        return f;
    }

    public <T> ForkJoinTask<T> externalSubmit(ForkJoinTask<T> task) { return submit(task); }
    public <T> ForkJoinTask<T> lazySubmit(ForkJoinTask<T> task) { return submit(task); }

    public int setParallelism(int size) { return 1; }

    public <T> List<Future<T>> invokeAllUninterruptibly(Collection<? extends Callable<T>> tasks) {
        try {
            return invokeAll(tasks);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return Collections.emptyList();
        }
    }

    public <T> List<Future<T>> invokeAll(Collection<? extends Callable<T>> tasks)
        throws InterruptedException {
        return super.invokeAll(tasks);
    }

    public <T> List<Future<T>> invokeAll(Collection<? extends Callable<T>> tasks,
                                         long timeout, TimeUnit unit) throws InterruptedException {
        return super.invokeAll(tasks, timeout, unit);
    }

    public <T> T invokeAny(Collection<? extends Callable<T>> tasks)
        throws InterruptedException, ExecutionException {
        return super.invokeAny(tasks);
    }

    public <T> T invokeAny(Collection<? extends Callable<T>> tasks, long timeout, TimeUnit unit)
        throws InterruptedException, ExecutionException, TimeoutException {
        return super.invokeAny(tasks, timeout, unit);
    }

    public ScheduledFuture<?> schedule(Runnable command, long delay, TimeUnit unit) {
        return schedule(() -> {
            command.run();
            return null;
        }, delay, unit);
    }

    public <V> ScheduledFuture<V> schedule(Callable<V> callable, long delay, TimeUnit unit) {
        return new ImmediateScheduledFuture<>(submit(callable));
    }

    public ScheduledFuture<?> scheduleAtFixedRate(Runnable command, long initialDelay, long period, TimeUnit unit) {
        return schedule(command, initialDelay, unit);
    }

    public ScheduledFuture<?> scheduleWithFixedDelay(Runnable command, long initialDelay, long delay, TimeUnit unit) {
        return schedule(command, initialDelay, unit);
    }

    public <V> ForkJoinTask<V> submitWithTimeout(Callable<V> task, long timeout, TimeUnit unit,
                                                 Consumer<? super ForkJoinTask<V>> timeoutAction) {
        ForkJoinTask<V> submitted = submit(task);
        return submitted;
    }

    public void cancelDelayedTasksOnShutdown() {}

    public ForkJoinWorkerThreadFactory getFactory() { return defaultForkJoinWorkerThreadFactory; }
    public UncaughtExceptionHandler getUncaughtExceptionHandler() { return null; }
    public int getParallelism() { return 1; }
    public static int getCommonPoolParallelism() { return 1; }
    public int getPoolSize() { return 1; }
    public boolean getAsyncMode() { return false; }
    public int getRunningThreadCount() { return 1; }
    public int getActiveThreadCount() { return 1; }
    public boolean isQuiescent() { return true; }
    public long getStealCount() { return 0L; }
    public long getQueuedTaskCount() { return 0L; }
    public int getQueuedSubmissionCount() { return 0; }
    public long getDelayedTaskCount() { return 0L; }
    public boolean hasQueuedSubmissions() { return false; }
    protected ForkJoinTask<?> pollSubmission() { return null; }
    protected int drainTasksTo(Collection<? super ForkJoinTask<?>> c) { return 0; }

    public String toString() { return "ForkJoinPool[parallelism=1]"; }

    public void shutdown() {}
    public List<Runnable> shutdownNow() { return new ArrayList<>(); }
    public boolean isTerminated() { return false; }
    public boolean isTerminating() { return false; }
    public boolean isShutdown() { return false; }

    public boolean awaitTermination(long timeout, TimeUnit unit) throws InterruptedException {
        return true;
    }

    public boolean awaitQuiescence(long timeout, TimeUnit unit) {
        return true;
    }

    public void close() {
        shutdown();
    }

    public static void managedBlock(ManagedBlocker blocker) throws InterruptedException {
        while (!blocker.isReleasable()) {
            if (blocker.block()) {
                return;
            }
        }
    }

    protected <T> RunnableFuture<T> newTaskFor(Runnable runnable, T value) {
        return super.newTaskFor(runnable, value);
    }

    protected <T> RunnableFuture<T> newTaskFor(Callable<T> callable) {
        return super.newTaskFor(callable);
    }

    private static final class ImmediateScheduledFuture<V> implements ScheduledFuture<V> {
        private final Future<V> delegate;

        private ImmediateScheduledFuture(Future<V> delegate) {
            this.delegate = delegate;
        }

        public long getDelay(TimeUnit unit) { return 0L; }
        public int compareTo(Delayed o) { return 0; }
        public boolean cancel(boolean mayInterruptIfRunning) { return delegate.cancel(mayInterruptIfRunning); }
        public boolean isCancelled() { return delegate.isCancelled(); }
        public boolean isDone() { return delegate.isDone(); }
        public V get() throws InterruptedException, ExecutionException { return delegate.get(); }
        public V get(long timeout, TimeUnit unit) throws InterruptedException, ExecutionException, TimeoutException {
            return delegate.get(timeout, unit);
        }
    }
}
