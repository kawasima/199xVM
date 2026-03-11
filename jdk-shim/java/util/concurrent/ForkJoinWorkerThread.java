package java.util.concurrent;

public class ForkJoinWorkerThread extends Thread {
    protected final ForkJoinPool pool;

    protected ForkJoinWorkerThread(ForkJoinPool pool) {
        this.pool = pool;
    }

    public ForkJoinPool getPool() {
        return pool;
    }

    public int getPoolIndex() {
        return 0;
    }
}
