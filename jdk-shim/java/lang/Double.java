package java.lang;

public final class Double extends Number implements Comparable<Double> {
    public static final double POSITIVE_INFINITY = 1.0d / 0.0d;
    public static final double NEGATIVE_INFINITY = -1.0d / 0.0d;
    public static final double NaN = 0.0d / 0.0d;
    public static final int MAX_EXPONENT = 1023;
    public static final int MIN_EXPONENT = -1022;
    public static final int SIZE = 64;
    public static final int BYTES = 8;
    public static final int PRECISION = 53;
    public static final Class<Double> TYPE = null;
    private final double value;

    public Double(double value) { this.value = value; }

    public static Double valueOf(double d) { return new Double(d); }

    @Override public int intValue() { return (int) value; }
    @Override public long longValue() { return (long) value; }
    @Override public float floatValue() { return (float) value; }
    @Override public double doubleValue() { return value; }

    @Override public String toString() { return toString(value); }

    public static native String toString(double d);

    @Override public int hashCode() { return (int) doubleToLongBits(value); }

    @Override
    public boolean equals(Object obj) {
        return (obj instanceof Double other) && doubleToLongBits(value) == doubleToLongBits(other.value);
    }

    @Override
    public int compareTo(Double anotherDouble) {
        return compare(this.value, anotherDouble.value);
    }

    public static int compare(double d1, double d2) {
        if (d1 < d2) return -1;
        if (d1 > d2) return 1;
        return 0;
    }

    public static boolean isInfinite(double v) {
        return v == POSITIVE_INFINITY || v == NEGATIVE_INFINITY;
    }

    public static boolean isNaN(double v) {
        return v != v;
    }

    public static boolean isFinite(double v) {
        return !isInfinite(v) && !isNaN(v);
    }

    public static native long doubleToLongBits(double value);
    public static native double longBitsToDouble(long bits);
}
