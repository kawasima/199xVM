package java.io;

import java.util.Formatter;

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
    public native void print(int i);

    public PrintStream format(String format, Object... args) {
        String s = new Formatter().format(format, args).toString();
        print(s);
        return this;
    }

    public PrintStream printf(String format, Object... args) {
        return format(format, args);
    }

    public PrintStream append(CharSequence csq) {
        print(csq == null ? "null" : csq.toString());
        return this;
    }

    public PrintStream append(CharSequence csq, int start, int end) {
        CharSequence seq = csq == null ? "null" : csq;
        print(seq.subSequence(start, end).toString());
        return this;
    }

    public PrintStream append(char c) {
        print(String.valueOf(c));
        return this;
    }
}
