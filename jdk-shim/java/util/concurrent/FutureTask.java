package java.util.concurrent;

public class FutureTask<V> implements RunnableFuture<V> {
    private final Callable<V> callable;
    private V result;
    private Throwable exception;
    private boolean done;
    private boolean cancelled;

    public FutureTask(Callable<V> callable) {
        if (callable == null) throw new NullPointerException();
        this.callable = callable;
    }

    public FutureTask(Runnable runnable, V result) {
        if (runnable == null) throw new NullPointerException();
        this.callable = () -> {
            runnable.run();
            return result;
        };
    }

    public boolean isCancelled() { return cancelled; }
    public boolean isDone() { return done; }

    public boolean cancel(boolean mayInterruptIfRunning) {
        if (done) return false;
        cancelled = true;
        done = true;
        done();
        return true;
    }

    public V get() throws InterruptedException, ExecutionException {
        if (!done) run();
        if (cancelled) throw new CancellationException();
        if (exception != null) throw new ExecutionException(exception);
        return result;
    }

    public V get(long timeout, TimeUnit unit)
        throws InterruptedException, ExecutionException, TimeoutException {
        return get();
    }

    public V resultNow() {
        if (state() != Future.State.SUCCESS) throw new IllegalStateException();
        return result;
    }

    public Throwable exceptionNow() {
        if (state() != Future.State.FAILED) throw new IllegalStateException();
        return exception;
    }

    public Future.State state() {
        if (!done) return Future.State.RUNNING;
        if (cancelled) return Future.State.CANCELLED;
        if (exception != null) return Future.State.FAILED;
        return Future.State.SUCCESS;
    }

    protected void done() {}

    protected void set(V v) {
        if (done) return;
        result = v;
        done = true;
        done();
    }

    protected void setException(Throwable t) {
        if (done) return;
        exception = t;
        done = true;
        done();
    }

    public void run() {
        if (done || cancelled) return;
        try {
            set(callable.call());
        } catch (Throwable t) {
            setException(t);
        }
    }

    protected boolean runAndReset() {
        if (done || cancelled) return false;
        try {
            callable.call();
            return true;
        } catch (Throwable t) {
            setException(t);
            return false;
        }
    }

    public String toString() {
        return "FutureTask[" + state() + "]";
    }
}
