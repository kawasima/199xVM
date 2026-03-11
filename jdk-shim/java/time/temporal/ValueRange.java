package java.time.temporal;

import java.time.DateTimeException;

public final class ValueRange {
    private final long minSmallest;
    private final long minLargest;
    private final long maxSmallest;
    private final long maxLargest;

    private ValueRange(long minSmallest, long minLargest, long maxSmallest, long maxLargest) {
        this.minSmallest = minSmallest;
        this.minLargest = minLargest;
        this.maxSmallest = maxSmallest;
        this.maxLargest = maxLargest;
    }

    public static ValueRange of(long min, long max) {
        return new ValueRange(min, min, max, max);
    }

    public static ValueRange of(long min, long maxSmallest, long maxLargest) {
        return new ValueRange(min, min, maxSmallest, maxLargest);
    }

    public static ValueRange of(long minSmallest, long minLargest, long maxSmallest, long maxLargest) {
        return new ValueRange(minSmallest, minLargest, maxSmallest, maxLargest);
    }

    public long getMinimum() { return minSmallest; }
    public long getLargestMinimum() { return minLargest; }
    public long getSmallestMaximum() { return maxSmallest; }
    public long getMaximum() { return maxLargest; }

    public boolean isFixed() { return minSmallest == minLargest && maxSmallest == maxLargest; }

    public boolean isValidValue(long value) { return value >= getMinimum() && value <= getMaximum(); }

    public int checkValidIntValue(long value, TemporalField field) {
        if (!isValidValue(value)) throw new DateTimeException("Invalid value");
        return (int) value;
    }

    public long checkValidValue(long value, TemporalField field) {
        if (!isValidValue(value)) throw new DateTimeException("Invalid value");
        return value;
    }
}
