package java.lang;

import java.util.ArrayList;
import java.util.stream.Stream;
import java.util.stream.StreamImpl;

public final class String implements CharSequence, Comparable<String> {
    // The actual string content is managed natively by the VM (NativePayload::JavaString).
    // This shim provides method signatures so that javac can compile against it
    // and the bytecode can call these methods at runtime.

    public String() {}
    public String(char[] value) {}
    public String(char[] value, int offset, int count) {}
    public String(byte[] bytes) {}
    public String(byte[] bytes, int offset, int length) {}
    public String(byte[] bytes, int offset, int length, String charsetName) {}
    public String(byte[] bytes, int offset, int length, java.nio.charset.Charset charset) {}
    public String(byte[] bytes, String charsetName) {}
    public String(byte[] bytes, java.nio.charset.Charset charset) {}
    public String(byte[] ascii, int hibyte, int offset, int count) {}

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
    public boolean startsWith(String prefix, int toffset) {
        if (toffset < 0 || toffset + prefix.length() > length()) return false;
        return substring(toffset, toffset + prefix.length()).equals(prefix);
    }
    public native boolean endsWith(String suffix);
    public native int indexOf(String str);
    public native int indexOf(int ch);
    public native int lastIndexOf(int ch);
    public native String trim();
    public native String toLowerCase();
    public native String toUpperCase();
    public native char[] toCharArray();
    public native byte[] getBytes();
    public void getChars(int srcBegin, int srcEnd, char[] dst, int dstBegin) {
        if (srcBegin < 0 || srcEnd < srcBegin || srcEnd > length()) throw new IndexOutOfBoundsException();
        for (int i = srcBegin; i < srcEnd; i++) {
            dst[dstBegin++] = charAt(i);
        }
    }

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

    public String repeat(int count) {
        if (count < 0) throw new IllegalArgumentException("count is negative: " + count);
        if (count == 0 || isEmpty()) return "";
        StringBuilder sb = new StringBuilder(length() * count);
        for (int i = 0; i < count; i++) {
            sb.append(this);
        }
        return sb.toString();
    }

    public Stream<String> lines() {
        ArrayList<String> out = new ArrayList<>();
        int n = length();
        int start = 0;
        for (int i = 0; i < n; i++) {
            char ch = charAt(i);
            if (ch == '\n' || ch == '\r') {
                out.add(substring(start, i));
                if (ch == '\r' && i + 1 < n && charAt(i + 1) == '\n') i++;
                start = i + 1;
            }
        }
        if (start <= n) out.add(substring(start, n));
        return new StreamImpl<>(out);
    }
}
