package java.lang;

public final class Long extends Number implements Comparable<Long> {
    private final long value;

    public Long(long value) {
        this.value = value;
    }

    public static Long valueOf(long l) {
        return new Long(l);
    }

    public static long parseLong(String s) {
        return (long) Integer.parseInt(s);
    }

    @Override public int intValue() { return (int) value; }
    @Override public long longValue() { return value; }
    @Override public float floatValue() { return (float) value; }
    @Override public double doubleValue() { return (double) value; }

    @Override
    public int hashCode() {
        return (int)(value ^ (value >>> 32));
    }

    @Override
    public boolean equals(Object obj) {
        if (obj instanceof Long other) {
            return value == other.value;
        }
        return false;
    }

    @Override
    public int compareTo(Long anotherLong) {
        return compare(this.value, anotherLong.value);
    }

    public static int compare(long x, long y) {
        return (x < y) ? -1 : ((x == y) ? 0 : 1);
    }

    @Override
    public String toString() {
        return toString(value);
    }

    public static String toString(long l) {
        return Integer.toString((int) l); // Simplified; sufficient for small values
    }
}
