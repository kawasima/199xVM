package java.lang;

public final class Short extends Number implements Comparable<Short> {
    public static final int SIZE = 16;
    public static final int BYTES = 2;
    public static final Class<Short> TYPE = null;
    private final short value;
    public Short(short value) { this.value = value; }
    public static Short valueOf(short s) { return new Short(s); }
    @Override public int intValue() { return value; }
    @Override public long longValue() { return value; }
    @Override public float floatValue() { return value; }
    @Override public double doubleValue() { return value; }
    @Override public int compareTo(Short another) { return value - another.value; }
    @Override public String toString() { return Integer.toString(value); }
}
