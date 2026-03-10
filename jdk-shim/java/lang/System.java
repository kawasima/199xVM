package java.lang;

import java.io.PrintStream;

public final class System {
    // These are resolved natively by the VM (resolve_static_field).
    public static PrintStream out;
    public static PrintStream err;

    private System() {}

    public static native void arraycopy(Object src, int srcPos, Object dest, int destPos, int length);
    public static native long currentTimeMillis();
    public static native int identityHashCode(Object x);
}
