package java.lang;

/**
 * Minimal StringBuilder for string concatenation support.
 * StringConcatFactory (invokedynamic) may be handled natively,
 * but some bytecode explicitly uses StringBuilder.
 */
public final class StringBuilder implements CharSequence {
    private char[] value;
    private int count;

    public StringBuilder() {
        value = new char[16];
    }

    public StringBuilder(int capacity) {
        value = new char[capacity];
    }

    public StringBuilder(String str) {
        this();
        append(str);
    }

    private void ensureCapacity(int minimumCapacity) {
        if (minimumCapacity > value.length) {
            int newCapacity = value.length * 2 + 2;
            if (newCapacity < minimumCapacity) newCapacity = minimumCapacity;
            char[] newValue = new char[newCapacity];
            for (int i = 0; i < count; i++) {
                newValue[i] = value[i];
            }
            value = newValue;
        }
    }

    public StringBuilder append(String str) {
        if (str == null) str = "null";
        int len = str.length();
        ensureCapacity(count + len);
        for (int i = 0; i < len; i++) {
            value[count++] = str.charAt(i);
        }
        return this;
    }

    public StringBuilder append(Object obj) {
        return append(String.valueOf(obj));
    }

    public StringBuilder append(int i) {
        return append(Integer.toString(i));
    }

    public StringBuilder append(long l) {
        return append(Long.toString(l));
    }

    public StringBuilder append(boolean b) {
        return append(b ? "true" : "false");
    }

    public StringBuilder append(char c) {
        ensureCapacity(count + 1);
        value[count++] = c;
        return this;
    }

    @Override
    public int length() {
        return count;
    }

    @Override
    public char charAt(int index) {
        return value[index];
    }

    @Override
    public CharSequence subSequence(int start, int end) {
        return toString().substring(start, end);
    }

    @Override
    public String toString() {
        return new String(value, 0, count);
    }
}
