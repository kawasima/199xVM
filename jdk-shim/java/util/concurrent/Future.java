package java.util.concurrent;

public interface Future<V> {
    public static enum State {
        RUNNING, SUCCESS, FAILED, CANCELLED
    }

    boolean cancel(boolean mayInterruptIfRunning);
    boolean isCancelled();
    boolean isDone();
    V get() throws InterruptedException, ExecutionException;
    V get(long timeout, TimeUnit unit) throws InterruptedException, ExecutionException, TimeoutException;

    default State state() {
        if (!isDone()) return State.RUNNING;
        if (isCancelled()) return State.CANCELLED;
        try {
            get();
            return State.SUCCESS;
        } catch (ExecutionException e) {
            return State.FAILED;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException(e);
        }
    }

    default V resultNow() {
        if (state() != State.SUCCESS) {
            throw new IllegalStateException();
        }
        try {
            return get();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException(e);
        } catch (ExecutionException e) {
            throw new IllegalStateException(e);
        }
    }

    default Throwable exceptionNow() {
        if (state() != State.FAILED) {
            throw new IllegalStateException();
        }
        try {
            get();
            throw new IllegalStateException();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException(e);
        } catch (ExecutionException e) {
            return e.getCause();
        }
    }
}
