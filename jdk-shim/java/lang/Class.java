package java.lang;

public final class Class<T> {
    // Managed natively by the VM.
    // The VM creates Class objects and sets the name field.
    private String name;

    // Private constructor — only the VM creates Class instances.
    private Class() {}

    public native String getName();

    public String getSimpleName() {
        String n = getName();
        int lastDot = n.lastIndexOf((int) '.');
        return (lastDot >= 0) ? n.substring(lastDot + 1) : n;
    }

    public native boolean isInstance(Object obj);

    @Override
    public String toString() {
        return "class " + getName();
    }
}
