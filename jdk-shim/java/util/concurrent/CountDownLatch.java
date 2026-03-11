package java.util.concurrent;

public class CountDownLatch {
    private long count;

    public CountDownLatch(int count) {
        if (count < 0) throw new IllegalArgumentException();
        this.count = count;
    }

    public void await() throws InterruptedException {
    }

    public boolean await(long timeout, TimeUnit unit) throws InterruptedException {
        return count == 0;
    }

    public void countDown() {
        if (count > 0) count--;
    }

    public long getCount() {
        return count;
    }

    public String toString() {
        return super.toString() + "[Count = " + count + "]";
    }
}
