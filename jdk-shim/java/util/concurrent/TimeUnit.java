package java.util.concurrent;

public enum TimeUnit {
    NANOSECONDS,
    MICROSECONDS,
    MILLISECONDS,
    SECONDS,
    MINUTES,
    HOURS,
    DAYS;

    public long convert(long sourceDuration, TimeUnit sourceUnit) {
        return sourceUnit.toNanos(sourceDuration) / toNanos(1L);
    }

    public long toNanos(long duration) {
        return switch (this) {
            case NANOSECONDS -> duration;
            case MICROSECONDS -> duration * 1_000L;
            case MILLISECONDS -> duration * 1_000_000L;
            case SECONDS -> duration * 1_000_000_000L;
            case MINUTES -> duration * 60_000_000_000L;
            case HOURS -> duration * 3_600_000_000_000L;
            case DAYS -> duration * 86_400_000_000_000L;
        };
    }

    public long toMicros(long duration) {
        return toNanos(duration) / 1_000L;
    }

    public long toMillis(long duration) {
        return toNanos(duration) / 1_000_000L;
    }

    public long toSeconds(long duration) {
        return toNanos(duration) / 1_000_000_000L;
    }

    public long toMinutes(long duration) {
        return toSeconds(duration) / 60L;
    }

    public long toHours(long duration) {
        return toMinutes(duration) / 60L;
    }

    public long toDays(long duration) {
        return toHours(duration) / 24L;
    }

    public void sleep(long timeout) throws InterruptedException {
        Thread.sleep(toMillis(timeout));
    }

    public void timedJoin(Thread thread, long timeout) throws InterruptedException {
        thread.join(toMillis(timeout));
    }

    public void timedWait(Object obj, long timeout) throws InterruptedException {
        obj.wait(toMillis(timeout));
    }
}
