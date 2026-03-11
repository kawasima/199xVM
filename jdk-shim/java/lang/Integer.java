package java.lang;

public final class Integer extends Number implements Comparable<Integer> {
    public static final int MIN_VALUE = 0x80000000;
    public static final int MAX_VALUE = 0x7fffffff;
    public static final int SIZE = 32;
    public static final int BYTES = 4;

    private final int value;

    public Integer(int value) {
        this.value = value;
    }

    public Integer(String s) {
        this.value = parseInt(s);
    }

    public static Integer valueOf(int i) {
        return new Integer(i);
    }

    public static Integer valueOf(String s) {
        return valueOf(parseInt(s));
    }

    public static Integer valueOf(String s, int radix) {
        return valueOf(parseInt(s, radix));
    }

    public static int parseInt(String s) {
        return parseInt(s, 10);
    }

    public static int parseInt(String s, int radix) {
        if (s == null) throw new NumberFormatException("null");
        int len = s.length();
        if (len == 0) throw new NumberFormatException(s);
        if (radix < 2 || radix > 36) throw new NumberFormatException("radix " + radix);

        boolean negative = false;
        int i = 0;
        char firstChar = s.charAt(0);
        if (firstChar == '-') {
            negative = true;
            i++;
        } else if (firstChar == '+') {
            i++;
        }
        if (i >= len) throw new NumberFormatException(s);

        int result = 0;
        while (i < len) {
            int digit = Character.digit(s.charAt(i++), radix);
            if (digit < 0) throw new NumberFormatException(s);
            result = result * radix + digit;
        }
        return negative ? -result : result;
    }

    public static Integer decode(String nm) {
        if (nm == null) throw new NumberFormatException("null");
        int len = nm.length();
        if (len == 0) throw new NumberFormatException(nm);

        int radix = 10;
        int index = 0;
        boolean negative = false;

        char firstChar = nm.charAt(0);
        if (firstChar == '-') {
            negative = true;
            index++;
        } else if (firstChar == '+') {
            index++;
        }

        if (index < len && nm.charAt(index) == '0') {
            if (index + 1 < len) {
                char second = nm.charAt(index + 1);
                if (second == 'x' || second == 'X') {
                    radix = 16;
                    index += 2;
                } else {
                    radix = 8;
                    index++;
                }
            }
        } else if (index < len && nm.charAt(index) == '#') {
            radix = 16;
            index++;
        }

        if (index >= len) throw new NumberFormatException(nm);

        String digits = nm.substring(index);
        int result = parseInt(digits, radix);
        return valueOf(negative ? -result : result);
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

    public byte byteValue() {
        return (byte) value;
    }

    public short shortValue() {
        return (short) value;
    }

    @Override
    public int hashCode() {
        return value;
    }

    public static int hashCode(int value) {
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

    public static int compareUnsigned(int x, int y) {
        return compare(x + MIN_VALUE, y + MIN_VALUE);
    }

    public static long toUnsignedLong(int x) {
        return ((long) x) & 0xffffffffL;
    }

    public static int divideUnsigned(int dividend, int divisor) {
        return (int) (toUnsignedLong(dividend) / toUnsignedLong(divisor));
    }

    public static int remainderUnsigned(int dividend, int divisor) {
        return (int) (toUnsignedLong(dividend) % toUnsignedLong(divisor));
    }

    public static int highestOneBit(int i) {
        i |= (i >> 1);
        i |= (i >> 2);
        i |= (i >> 4);
        i |= (i >> 8);
        i |= (i >> 16);
        return i - (i >>> 1);
    }

    public static int lowestOneBit(int i) {
        return i & -i;
    }

    public static int numberOfLeadingZeros(int i) {
        if (i <= 0) return i == 0 ? 32 : 0;
        int n = 31;
        if (i >= 1 << 16) { n -= 16; i >>>= 16; }
        if (i >= 1 << 8)  { n -= 8;  i >>>= 8; }
        if (i >= 1 << 4)  { n -= 4;  i >>>= 4; }
        if (i >= 1 << 2)  { n -= 2;  i >>>= 2; }
        return n - (i >>> 1);
    }

    public static int numberOfTrailingZeros(int i) {
        if (i == 0) return 32;
        int n = 31;
        int y;
        y = i << 16; if (y != 0) { n -= 16; i = y; }
        y = i << 8;  if (y != 0) { n -= 8;  i = y; }
        y = i << 4;  if (y != 0) { n -= 4;  i = y; }
        y = i << 2;  if (y != 0) { n -= 2;  i = y; }
        return n - ((i << 1) >>> 31);
    }

    public static int bitCount(int i) {
        i = i - ((i >>> 1) & 0x55555555);
        i = (i & 0x33333333) + ((i >>> 2) & 0x33333333);
        i = (i + (i >>> 4)) & 0x0f0f0f0f;
        i = i + (i >>> 8);
        i = i + (i >>> 16);
        return i & 0x3f;
    }

    public static int rotateLeft(int i, int distance) {
        return (i << distance) | (i >>> -distance);
    }

    public static int rotateRight(int i, int distance) {
        return (i >>> distance) | (i << -distance);
    }

    public static int reverse(int i) {
        i = (i & 0x55555555) << 1 | (i >>> 1) & 0x55555555;
        i = (i & 0x33333333) << 2 | (i >>> 2) & 0x33333333;
        i = (i & 0x0f0f0f0f) << 4 | (i >>> 4) & 0x0f0f0f0f;
        return reverseBytes(i);
    }

    public static int signum(int i) {
        return (i >> 31) | (-i >>> 31);
    }

    public static int reverseBytes(int i) {
        return (i << 24) |
               ((i & 0xff00) << 8) |
               ((i >>> 8) & 0xff00) |
               (i >>> 24);
    }

    public static int sum(int a, int b) {
        return a + b;
    }

    public static int max(int a, int b) {
        return (a >= b) ? a : b;
    }

    public static int min(int a, int b) {
        return (a <= b) ? a : b;
    }

    @Override
    public String toString() {
        return toString(value);
    }

    public static String toString(int i) {
        if (i == MIN_VALUE) return "-2147483648";
        if (i == 0) return "0";
        boolean negative = i < 0;
        if (negative) i = -i;
        char[] buf = new char[11];
        int pos = buf.length;
        while (i > 0) {
            buf[--pos] = (char) ('0' + (i % 10));
            i /= 10;
        }
        if (negative) buf[--pos] = '-';
        return new String(buf, pos, buf.length - pos);
    }

    public static String toString(int i, int radix) {
        if (radix < 2 || radix > 36) radix = 10;
        if (radix == 10) return toString(i);
        boolean negative = i < 0;
        if (!negative) i = -i;
        char[] buf = new char[33];
        int pos = buf.length;
        while (i <= -radix) {
            buf[--pos] = digitChar(-(i % radix));
            i /= radix;
        }
        buf[--pos] = digitChar(-i);
        if (negative) buf[--pos] = '-';
        return new String(buf, pos, buf.length - pos);
    }

    public static String toHexString(int i) {
        return toUnsignedString(i, 16);
    }

    public static String toOctalString(int i) {
        return toUnsignedString(i, 8);
    }

    public static String toBinaryString(int i) {
        return toUnsignedString(i, 2);
    }

    public static String toUnsignedString(int i) {
        return toUnsignedString(i, 10);
    }

    public static String toUnsignedString(int i, int radix) {
        if (radix < 2 || radix > 36) radix = 10;
        if (i == 0) return "0";
        char[] buf = new char[33];
        int pos = buf.length;
        if (radix == 10) {
            // Use long to handle unsigned
            long val = toUnsignedLong(i);
            while (val > 0) {
                buf[--pos] = (char) ('0' + (val % 10));
                val /= 10;
            }
        } else if ((radix & (radix - 1)) == 0) {
            // Power of 2 radix — use bit masking
            int shift = numberOfTrailingZeros(radix);
            int mask = radix - 1;
            do {
                buf[--pos] = digitChar(i & mask);
                i >>>= shift;
            } while (i != 0);
        } else {
            long val = toUnsignedLong(i);
            while (val > 0) {
                buf[--pos] = digitChar((int) (val % radix));
                val /= radix;
            }
        }
        return new String(buf, pos, buf.length - pos);
    }

    private static char digitChar(int digit) {
        if (digit < 10) return (char) ('0' + digit);
        return (char) ('a' + digit - 10);
    }
}
