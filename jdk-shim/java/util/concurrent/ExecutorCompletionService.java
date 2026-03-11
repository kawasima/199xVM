package java.util.concurrent;

public class ExecutorCompletionService<V> implements CompletionService<V> {
    private final Executor executor;
    private final BlockingQueue<Future<V>> completionQueue;

    public ExecutorCompletionService(Executor executor) {
        this(executor, new LinkedBlockingQueue<>());
    }

    public ExecutorCompletionService(Executor executor, BlockingQueue<Future<V>> completionQueue) {
        if (executor == null || completionQueue == null) {
            throw new NullPointerException();
        }
        this.executor = executor;
        this.completionQueue = completionQueue;
    }

    public Future<V> submit(Callable<V> task) {
        if (task == null) throw new NullPointerException();
        Future<V> f;
        if (executor instanceof ExecutorService es) {
            f = es.submit(task);
        } else {
            FutureTask<V> ft = new FutureTask<>(task);
            executor.execute(ft);
            f = ft;
        }
        completionQueue.offer(f);
        return f;
    }

    public Future<V> submit(Runnable task, V result) {
        if (task == null) throw new NullPointerException();
        Future<V> f;
        if (executor instanceof ExecutorService es) {
            f = es.submit(task, result);
        } else {
            FutureTask<V> ft = new FutureTask<>(task, result);
            executor.execute(ft);
            f = ft;
        }
        completionQueue.offer(f);
        return f;
    }

    public Future<V> take() throws InterruptedException {
        return completionQueue.take();
    }

    public Future<V> poll() {
        return completionQueue.poll();
    }

    public Future<V> poll(long timeout, TimeUnit unit) throws InterruptedException {
        return completionQueue.poll(timeout, unit);
    }
}
