package java.lang;

import java.io.Serializable;

public final class Boolean implements Serializable, Comparable<Boolean> {
    public static final Boolean TRUE = new Boolean(true);
    public static final Boolean FALSE = new Boolean(false);
    @SuppressWarnings("unchecked")
    public static final Class<Boolean> TYPE = (Class<Boolean>) primitiveType("boolean");
    private static Class<?> primitiveType(String name) { try { return Class.forName(name); } catch (ClassNotFoundException e) { return null; } }

    private final boolean value;

    public Boolean(boolean value) {
        this.value = value;
    }

    public static Boolean valueOf(boolean b) {
        return b ? TRUE : FALSE;
    }

    public boolean booleanValue() {
        return value;
    }

    @Override
    public String toString() {
        return value ? "true" : "false";
    }

    @Override
    public int hashCode() {
        return value ? 1231 : 1237;
    }

    @Override
    public boolean equals(Object obj) {
        if (obj instanceof Boolean other) {
            return value == other.value;
        }
        return false;
    }

    @Override
    public int compareTo(Boolean b) {
        return compare(this.value, b.value);
    }

    public static int compare(boolean x, boolean y) {
        return (x == y) ? 0 : (x ? 1 : -1);
    }
}
