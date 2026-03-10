package java.lang;

public final class Integer extends Number implements Comparable<Integer> {
    private final int value;

    public Integer(int value) {
        this.value = value;
    }

    public static Integer valueOf(int i) {
        return new Integer(i);
    }

    public static int parseInt(String s) {
        // Minimal implementation
        if (s == null) throw new NumberFormatException("null");
        int result = 0;
        boolean negative = false;
        int i = 0;
        int len = s.length();
        if (len == 0) throw new NumberFormatException(s);
        char firstChar = s.charAt(0);
        if (firstChar == '-') {
            negative = true;
            i++;
        } else if (firstChar == '+') {
            i++;
        }
        while (i < len) {
            int digit = s.charAt(i++) - '0';
            if (digit < 0 || digit > 9) throw new NumberFormatException(s);
            result = result * 10 + digit;
        }
        return negative ? -result : result;
    }

    @Override
    public int intValue() {
        return value;
    }

    @Override
    public long longValue() {
        return (long) value;
    }

    @Override
    public float floatValue() {
        return (float) value;
    }

    @Override
    public double doubleValue() {
        return (double) value;
    }

    @Override
    public int hashCode() {
        return value;
    }

    @Override
    public boolean equals(Object obj) {
        if (obj instanceof Integer other) {
            return value == other.value;
        }
        return false;
    }

    @Override
    public int compareTo(Integer anotherInteger) {
        return compare(this.value, anotherInteger.value);
    }

    public static int compare(int x, int y) {
        return (x < y) ? -1 : ((x == y) ? 0 : 1);
    }

    @Override
    public String toString() {
        return toString(value);
    }

    public static String toString(int i) {
        if (i == 0) return "0";
        boolean negative = i < 0;
        if (negative) i = -i;
        // Build digits in reverse
        char[] buf = new char[11];
        int pos = buf.length;
        while (i > 0) {
            buf[--pos] = (char) ('0' + (i % 10));
            i /= 10;
        }
        if (negative) buf[--pos] = '-';
        return new String(buf, pos, buf.length - pos);
    }

    public static String toHexString(int i) {
        if (i == 0) return "0";
        char[] buf = new char[8];
        int pos = buf.length;
        while (i != 0) {
            int digit = i & 0xf;
            buf[--pos] = (char) (digit < 10 ? '0' + digit : 'a' + digit - 10);
            i >>>= 4;
        }
        return new String(buf, pos, buf.length - pos);
    }
}
