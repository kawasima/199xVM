package java.lang;

public final class Float extends Number implements Comparable<Float> {
    private final float value;

    public Float(float value) { this.value = value; }

    public static Float valueOf(float f) { return new Float(f); }

    @Override public int intValue() { return (int) value; }
    @Override public long longValue() { return (long) value; }
    @Override public float floatValue() { return value; }
    @Override public double doubleValue() { return (double) value; }

    @Override public String toString() { return toString(value); }

    public static native String toString(float f);

    @Override public int hashCode() { return floatToIntBits(value); }

    @Override
    public boolean equals(Object obj) {
        return (obj instanceof Float other) && floatToIntBits(value) == floatToIntBits(other.value);
    }

    @Override
    public int compareTo(Float anotherFloat) {
        return compare(this.value, anotherFloat.value);
    }

    public static int compare(float f1, float f2) {
        if (f1 < f2) return -1;
        if (f1 > f2) return 1;
        return 0;
    }

    public static native int floatToIntBits(float value);
}
