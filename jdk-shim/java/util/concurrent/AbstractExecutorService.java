package java.util.concurrent;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

public abstract class AbstractExecutorService implements ExecutorService {
    protected <T> RunnableFuture<T> newTaskFor(Runnable runnable, T value) {
        return new SimpleFutureTask<>(runnable, value);
    }

    protected <T> RunnableFuture<T> newTaskFor(Callable<T> callable) {
        return new SimpleFutureTask<>(callable);
    }

    public Future<?> submit(Runnable task) {
        return submit(task, null);
    }

    public <T> Future<T> submit(Runnable task, T result) {
        RunnableFuture<T> f = newTaskFor(task, result);
        execute(f);
        return f;
    }

    public <T> Future<T> submit(Callable<T> task) {
        RunnableFuture<T> f = newTaskFor(task);
        execute(f);
        return f;
    }

    public <T> List<Future<T>> invokeAll(Collection<? extends Callable<T>> tasks) throws InterruptedException {
        List<Future<T>> futures = new ArrayList<>();
        for (Callable<T> task : tasks) {
            Future<T> f = submit(task);
            futures.add(f);
        }
        for (Future<T> f : futures) {
            try {
                f.get();
            } catch (ExecutionException | CancellationException ignored) {
            }
        }
        return futures;
    }

    public <T> List<Future<T>> invokeAll(Collection<? extends Callable<T>> tasks, long timeout, TimeUnit unit)
        throws InterruptedException {
        return invokeAll(tasks);
    }

    public <T> T invokeAny(Collection<? extends Callable<T>> tasks)
        throws InterruptedException, ExecutionException {
        for (Callable<T> task : tasks) {
            return submit(task).get();
        }
        throw new ExecutionException(new IllegalStateException("No tasks"));
    }

    public <T> T invokeAny(Collection<? extends Callable<T>> tasks, long timeout, TimeUnit unit)
        throws InterruptedException, ExecutionException, TimeoutException {
        return invokeAny(tasks);
    }

    private static final class SimpleFutureTask<T> implements RunnableFuture<T> {
        private final Callable<T> callable;
        private T result;
        private Throwable exception;
        private boolean done;
        private boolean cancelled;

        private SimpleFutureTask(Callable<T> callable) {
            this.callable = callable;
        }

        private SimpleFutureTask(Runnable runnable, T value) {
            this.callable = () -> {
                runnable.run();
                return value;
            };
        }

        public void run() {
            if (done || cancelled) return;
            try {
                result = callable.call();
            } catch (Throwable t) {
                exception = t;
            } finally {
                done = true;
            }
        }

        public boolean cancel(boolean mayInterruptIfRunning) {
            if (done) return false;
            cancelled = true;
            done = true;
            return true;
        }

        public boolean isCancelled() { return cancelled; }
        public boolean isDone() { return done; }

        public T get() throws InterruptedException, ExecutionException {
            if (!done) run();
            if (cancelled) throw new CancellationException();
            if (exception != null) throw new ExecutionException(exception);
            return result;
        }

        public T get(long timeout, TimeUnit unit) throws InterruptedException, ExecutionException, TimeoutException {
            return get();
        }
    }
}
