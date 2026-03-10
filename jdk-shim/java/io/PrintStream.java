package java.io;

/**
 * Minimal PrintStream stub.
 * println/print are handled natively by the VM.
 */
public class PrintStream {
    public native void println(String s);
    public native void println(Object o);
    public native void println(int i);
    public native void println();
    public native void print(String s);
    public native void print(Object o);
}
