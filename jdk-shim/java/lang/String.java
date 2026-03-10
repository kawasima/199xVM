package java.lang;

public final class String implements CharSequence, Comparable<String> {
    // The actual string content is managed natively by the VM (NativePayload::JavaString).
    // This shim provides method signatures so that javac can compile against it
    // and the bytecode can call these methods at runtime.

    public String() {}
    public String(char[] value) {}
    public String(char[] value, int offset, int count) {}
    public String(byte[] bytes) {}

    // Native — delegated to Rust
    public native int length();
    public native char charAt(int index);
    public native boolean isEmpty();
    public native boolean equals(Object anObject);
    public native int hashCode();
    public native String substring(int beginIndex);
    public native String substring(int beginIndex, int endIndex);
    public native String concat(String str);
    public native boolean contains(CharSequence s);
    public native boolean startsWith(String prefix);
    public native boolean endsWith(String suffix);
    public native int indexOf(String str);
    public native int indexOf(int ch);
    public native int lastIndexOf(int ch);
    public native String trim();
    public native String toLowerCase();
    public native String toUpperCase();
    public native char[] toCharArray();
    public native byte[] getBytes();

    @Override
    public native String toString();

    @Override
    public CharSequence subSequence(int start, int end) {
        return substring(start, end);
    }

    @Override
    public int compareTo(String anotherString) {
        // Native
        return 0;
    }

    public static String valueOf(Object obj) {
        return (obj == null) ? "null" : obj.toString();
    }

    public static String valueOf(int i) {
        return Integer.toString(i);
    }

    public static String valueOf(long l) {
        return Long.toString(l);
    }

    public static String valueOf(boolean b) {
        return b ? "true" : "false";
    }

    public static String valueOf(char c) {
        return "" + c;
    }

    public static String valueOf(double d) {
        return Double.toString(d);
    }

    public static String valueOf(float f) {
        return Float.toString(f);
    }

    public String formatted(Object... args) {
        return new java.util.Formatter().format(this, args).toString();
    }
}
