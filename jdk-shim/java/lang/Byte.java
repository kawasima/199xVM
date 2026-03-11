package java.lang;

public final class Byte extends Number implements Comparable<Byte> {
    public static final int SIZE = 8;
    public static final int BYTES = 1;
    @SuppressWarnings("unchecked")
    public static final Class<Byte> TYPE = (Class<Byte>) primitiveType("byte");
    private static Class<?> primitiveType(String name) { try { return Class.forName(name); } catch (ClassNotFoundException e) { return null; } }
    private final byte value;
    public Byte(byte value) { this.value = value; }
    public static Byte valueOf(byte b) { return new Byte(b); }
    @Override public int intValue() { return value; }
    @Override public long longValue() { return value; }
    @Override public float floatValue() { return value; }
    @Override public double doubleValue() { return value; }
    @Override public int compareTo(Byte another) { return value - another.value; }
    @Override public String toString() { return Integer.toString(value); }
}
