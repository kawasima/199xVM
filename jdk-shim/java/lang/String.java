package java.lang;

import java.io.Serializable;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Locale;
import java.util.Optional;
import java.util.function.Function;
import java.util.stream.Stream;
import java.util.stream.StreamImpl;

public final class String implements CharSequence, Comparable<String>, Serializable {
    public static final Comparator<String> CASE_INSENSITIVE_ORDER = new Comparator<String>() {
        public int compare(String a, String b) {
            return a.compareToIgnoreCase(b);
        }
    };

    // String payload is stored natively in VM (NativePayload::JavaString).
    public String() {}
    public String(String original) {}
    public String(char[] value) {}
    public String(char[] value, int offset, int count) {}
    public String(int[] codePoints, int offset, int count) {}
    public String(byte[] bytes) {}
    public String(byte[] bytes, int offset, int length) {}
    public String(byte[] bytes, int offset, int length, String charsetName) {}
    public String(byte[] bytes, int offset, int length, java.nio.charset.Charset charset) {}
    public String(byte[] bytes, String charsetName) {}
    public String(byte[] bytes, java.nio.charset.Charset charset) {}
    public String(byte[] ascii, int hibyte, int offset, int count) {}
    public String(byte[] ascii, int hibyte) {}
    public String(StringBuffer buffer) {}
    public String(StringBuilder builder) {}

    // Native core
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

    public int codePointAt(int index) {
        return Character.codePointAt(this, index);
    }

    public int codePointBefore(int index) {
        return Character.codePointBefore(this, index);
    }

    public int codePointCount(int beginIndex, int endIndex) {
        return Character.codePointCount(this, beginIndex, endIndex);
    }

    public int offsetByCodePoints(int index, int codePointOffset) {
        int i = index;
        if (codePointOffset >= 0) {
            for (int c = 0; c < codePointOffset; c++) {
                if (i >= length()) throw new IndexOutOfBoundsException();
                i++;
            }
        } else {
            for (int c = 0; c < -codePointOffset; c++) {
                if (i <= 0) throw new IndexOutOfBoundsException();
                i--;
            }
        }
        return i;
    }

    public void getChars(int srcBegin, int srcEnd, char[] dst, int dstBegin) {
        if (srcBegin < 0 || srcEnd < srcBegin || srcEnd > length()) throw new IndexOutOfBoundsException();
        for (int i = srcBegin; i < srcEnd; i++) {
            dst[dstBegin++] = charAt(i);
        }
    }

    public void getBytes(int srcBegin, int srcEnd, byte[] dst, int dstBegin) {
        if (srcBegin < 0 || srcEnd < srcBegin || srcEnd > length()) throw new IndexOutOfBoundsException();
        for (int i = srcBegin; i < srcEnd; i++) {
            dst[dstBegin++] = (byte) charAt(i);
        }
    }

    public byte[] getBytes(String charsetName) {
        return getBytes();
    }

    public byte[] getBytes(java.nio.charset.Charset charset) {
        return getBytes();
    }

    public boolean contentEquals(StringBuffer sb) {
        return sb != null && contentEquals((CharSequence) sb);
    }

    public boolean contentEquals(CharSequence cs) {
        if (cs == null || cs.length() != length()) return false;
        for (int i = 0; i < length(); i++) {
            if (charAt(i) != cs.charAt(i)) return false;
        }
        return true;
    }

    public boolean equalsIgnoreCase(String anotherString) {
        return compareToIgnoreCase(anotherString) == 0;
    }

    public int compareTo(String anotherString) {
        int n1 = length();
        int n2 = anotherString.length();
        int lim = n1 < n2 ? n1 : n2;
        for (int i = 0; i < lim; i++) {
            char c1 = charAt(i);
            char c2 = anotherString.charAt(i);
            if (c1 != c2) return c1 - c2;
        }
        return n1 - n2;
    }

    public int compareToIgnoreCase(String str) {
        int n1 = length();
        int n2 = str.length();
        int lim = n1 < n2 ? n1 : n2;
        for (int i = 0; i < lim; i++) {
            char c1 = Character.toLowerCase(charAt(i));
            char c2 = Character.toLowerCase(str.charAt(i));
            if (c1 != c2) return c1 - c2;
        }
        return n1 - n2;
    }

    public boolean regionMatches(int toffset, String other, int ooffset, int len) {
        return regionMatches(false, toffset, other, ooffset, len);
    }

    public boolean regionMatches(boolean ignoreCase, int toffset, String other, int ooffset, int len) {
        if (toffset < 0 || ooffset < 0 || len < 0) return false;
        if (toffset + len > length() || ooffset + len > other.length()) return false;
        for (int i = 0; i < len; i++) {
            char c1 = charAt(toffset + i);
            char c2 = other.charAt(ooffset + i);
            if (ignoreCase) {
                c1 = Character.toLowerCase(c1);
                c2 = Character.toLowerCase(c2);
            }
            if (c1 != c2) return false;
        }
        return true;
    }

    public boolean startsWith(String prefix, int toffset) {
        return regionMatches(toffset, prefix, 0, prefix.length());
    }

    public int indexOf(String str, int fromIndex) {
        if (str == null) return -1;
        if (fromIndex < 0) fromIndex = 0;
        int n = length();
        int m = str.length();
        if (m == 0) return fromIndex <= n ? fromIndex : n;
        if (m > n) return -1;
        for (int i = fromIndex; i <= n - m; i++) {
            if (regionMatches(i, str, 0, m)) return i;
        }
        return -1;
    }

    public int indexOf(int ch, int fromIndex) {
        if (fromIndex < 0) fromIndex = 0;
        for (int i = fromIndex; i < length(); i++) {
            if (charAt(i) == (char) ch) return i;
        }
        return -1;
    }

    public int indexOf(int ch, int beginIndex, int endIndex) {
        if (beginIndex < 0) beginIndex = 0;
        if (endIndex > length()) endIndex = length();
        for (int i = beginIndex; i < endIndex; i++) {
            if (charAt(i) == (char) ch) return i;
        }
        return -1;
    }

    public int indexOf(String str, int beginIndex, int endIndex) {
        if (beginIndex < 0) beginIndex = 0;
        if (endIndex > length()) endIndex = length();
        int max = endIndex - str.length();
        for (int i = beginIndex; i <= max; i++) {
            if (regionMatches(i, str, 0, str.length())) return i;
        }
        return -1;
    }

    public int lastIndexOf(int ch, int fromIndex) {
        if (fromIndex >= length()) fromIndex = length() - 1;
        for (int i = fromIndex; i >= 0; i--) {
            if (charAt(i) == (char) ch) return i;
        }
        return -1;
    }

    public int lastIndexOf(String str) {
        return lastIndexOf(str, length());
    }

    public int lastIndexOf(String str, int fromIndex) {
        int m = str.length();
        if (m == 0) return fromIndex < length() ? fromIndex : length();
        if (fromIndex > length() - m) fromIndex = length() - m;
        for (int i = fromIndex; i >= 0; i--) {
            if (regionMatches(i, str, 0, m)) return i;
        }
        return -1;
    }

    @Override
    public CharSequence subSequence(int start, int end) {
        return substring(start, end);
    }

    public String replace(char oldChar, char newChar) {
        if (oldChar == newChar) return this;
        StringBuilder sb = new StringBuilder(length());
        for (int i = 0; i < length(); i++) {
            char c = charAt(i);
            sb.append(c == oldChar ? newChar : c);
        }
        return sb.toString();
    }

    public String replace(CharSequence target, CharSequence replacement) {
        return replace(target.toString(), replacement.toString());
    }

    public String replace(String target, String replacement) {
        if (target == null || replacement == null) throw new NullPointerException();
        if (target.length() == 0) return this;
        StringBuilder sb = new StringBuilder();
        int from = 0;
        int at;
        while ((at = indexOf(target, from)) >= 0) {
            sb.append(substring(from, at));
            sb.append(replacement);
            from = at + target.length();
        }
        sb.append(substring(from));
        return sb.toString();
    }

    public String[] split(String regex) {
        return split(regex, 0);
    }

    public String[] split(String regex, int limit) {
        if (regex == null) throw new NullPointerException();
        if (regex.length() == 0) return new String[] { this };
        ArrayList<String> out = new ArrayList<>();
        int from = 0;
        int at;
        while ((at = indexOf(regex, from)) >= 0) {
            if (limit > 0 && out.size() + 1 >= limit) break;
            out.add(substring(from, at));
            from = at + regex.length();
        }
        out.add(substring(from));
        return out.toArray(new String[out.size()]);
    }

    public String[] splitWithDelimiters(String regex, int limit) {
        return split(regex, limit);
    }

    public boolean matches(String regex) {
        return java.util.regex.Pattern.matches(regex, this);
    }

    public String replaceFirst(String regex, String replacement) {
        return java.util.regex.Pattern.compile(regex).matcher(this).replaceAll(replacement);
    }

    public String replaceAll(String regex, String replacement) {
        return java.util.regex.Pattern.compile(regex).matcher(this).replaceAll(replacement);
    }

    public static String join(CharSequence delimiter, CharSequence... elements) {
        if (delimiter == null || elements == null) throw new NullPointerException();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < elements.length; i++) {
            if (i > 0) sb.append(delimiter);
            CharSequence e = elements[i];
            sb.append(e == null ? "null" : e.toString());
        }
        return sb.toString();
    }

    public static String join(CharSequence delimiter, Iterable<? extends CharSequence> elements) {
        if (delimiter == null || elements == null) throw new NullPointerException();
        StringBuilder sb = new StringBuilder();
        boolean first = true;
        for (CharSequence e : elements) {
            if (!first) sb.append(delimiter);
            first = false;
            sb.append(e == null ? "null" : e.toString());
        }
        return sb.toString();
    }

    public String toLowerCase(Locale locale) { return toLowerCase(); }
    public String toUpperCase(Locale locale) { return toUpperCase(); }

    public String strip() {
        return trim();
    }

    public String stripLeading() {
        int i = 0;
        while (i < length() && Character.isWhitespace(charAt(i))) i++;
        return substring(i);
    }

    public String stripTrailing() {
        int i = length() - 1;
        while (i >= 0 && Character.isWhitespace(charAt(i))) i--;
        return substring(0, i + 1);
    }

    public boolean isBlank() {
        for (int i = 0; i < length(); i++) {
            if (!Character.isWhitespace(charAt(i))) return false;
        }
        return true;
    }

    public String indent(int n) {
        if (n == 0) return this;
        String[] lines = split("\n", 0);
        String pad = n > 0 ? " ".repeat(n) : "";
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < lines.length; i++) {
            String line = lines[i];
            if (n > 0) {
                sb.append(pad).append(line);
            } else {
                int cut = -n;
                int j = 0;
                while (j < line.length() && j < cut && line.charAt(j) == ' ') j++;
                sb.append(line.substring(j));
            }
            if (i + 1 < lines.length) sb.append('\n');
        }
        return sb.toString();
    }

    public String stripIndent() {
        return this;
    }

    public String translateEscapes() {
        return this;
    }

    public <R> R transform(Function<? super String, ? extends R> f) {
        return f.apply(this);
    }

    @Override
    public native String toString();

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

    public static String format(String format, Object... args) {
        return new java.util.Formatter().format(format, args).toString();
    }

    public static String format(Locale locale, String format, Object... args) {
        return new java.util.Formatter().format(format, args).toString();
    }

    public String formatted(Object... args) {
        return new java.util.Formatter().format(this, args).toString();
    }

    public static String valueOf(Object obj) {
        return (obj == null) ? "null" : obj.toString();
    }

    public static String valueOf(char[] data) {
        return new String(data);
    }

    public static String valueOf(char[] data, int offset, int count) {
        return new String(data, offset, count);
    }

    public static String copyValueOf(char[] data, int offset, int count) {
        return valueOf(data, offset, count);
    }

    public static String copyValueOf(char[] data) {
        return valueOf(data);
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

    public native String intern();

    public String repeat(int count) {
        if (count < 0) throw new IllegalArgumentException("count is negative: " + count);
        if (count == 0 || isEmpty()) return "";
        StringBuilder sb = new StringBuilder(length() * count);
        for (int i = 0; i < count; i++) {
            sb.append(this);
        }
        return sb.toString();
    }

    public Optional<String> describeConstable() {
        return Optional.of(this);
    }
}
