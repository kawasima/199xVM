package java.lang;

public class Object {
    public Object() {}

    // Native — implemented in Rust
    public native int hashCode();

    public boolean equals(Object obj) {
        return (this == obj);
    }

    public String toString() {
        return getClass().getName() + "@" + Integer.toHexString(hashCode());
    }

    // Native — implemented in Rust
    public final native Class<?> getClass();

    protected native Object clone() throws CloneNotSupportedException;
}
