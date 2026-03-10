package java.lang;

import java.io.Serializable;

public abstract class Enum<E extends Enum<E>> implements Comparable<E>, Serializable {
    private final String name;
    private final int ordinal;

    protected Enum(String name, int ordinal) {
        this.name = name;
        this.ordinal = ordinal;
    }

    public final String name() { return name; }
    public final int ordinal() { return ordinal; }

    @Override
    public String toString() { return name; }

    @Override
    public final int compareTo(E o) {
        return this.ordinal() - o.ordinal();
    }

    @Override
    public final boolean equals(Object other) {
        return this == other;
    }

    @Override
    public final int hashCode() {
        return super.hashCode();
    }

    public static <T extends Enum<T>> T valueOf(Class<T> enumClass, String name) {
        // The VM handles enum constant resolution natively.
        throw new IllegalArgumentException("No enum constant " + name);
    }
}
