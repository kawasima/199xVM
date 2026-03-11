package java.lang;

public final class Long extends Number implements Comparable<Long> {
    public static final long MIN_VALUE = 0x8000000000000000L;
    public static final long MAX_VALUE = 0x7fffffffffffffffL;
    public static final int SIZE = 64;
    public static final int BYTES = 8;
    @SuppressWarnings("unchecked")
    public static final Class<Long> TYPE = (Class<Long>) primitiveType("long");
    private static Class<?> primitiveType(String name) { try { return Class.forName(name); } catch (ClassNotFoundException e) { return null; } }

    private final long value;

    public Long(long value) {
        this.value = value;
    }

    public Long(String s) {
        this.value = parseLong(s);
    }

    public static Long valueOf(long l) {
        return new Long(l);
    }

    public static Long valueOf(String s) {
        return valueOf(parseLong(s));
    }

    public static Long valueOf(String s, int radix) {
        return valueOf(parseLong(s, radix));
    }

    public static long parseLong(String s) {
        return parseLong(s, 10);
    }

    public static long parseLong(String s, int radix) {
        if (s == null || s.isEmpty()) {
            throw new NumberFormatException("null or empty string");
        }
        boolean negative = false;
        int i = 0;
        int len = s.length();
        if (s.charAt(0) == '-') {
            negative = true;
            i = 1;
        } else if (s.charAt(0) == '+') {
            i = 1;
        }
        if (i >= len) {
            throw new NumberFormatException("For input string: \"" + s + "\"");
        }
        long result = 0;
        while (i < len) {
            int digit = Character.digit(s.charAt(i), radix);
            if (digit < 0) {
                throw new NumberFormatException("For input string: \"" + s + "\"");
            }
            result = result * radix + digit;
            i++;
        }
        return negative ? -result : result;
    }

    public static Long decode(String nm) {
        if (nm == null || nm.isEmpty()) {
            throw new NumberFormatException("null or empty string");
        }
        int radix = 10;
        int index = 0;
        boolean negative = false;
        if (nm.charAt(0) == '-') {
            negative = true;
            index++;
        } else if (nm.charAt(0) == '+') {
            index++;
        }
        if (nm.startsWith("0x", index) || nm.startsWith("0X", index)) {
            index += 2;
            radix = 16;
        } else if (nm.startsWith("#", index)) {
            index += 1;
            radix = 16;
        } else if (nm.startsWith("0", index) && nm.length() > index + 1) {
            index += 1;
            radix = 8;
        }
        String sub = nm.substring(index);
        if (negative) {
            sub = "-" + sub;
        }
        return valueOf(parseLong(sub, radix));
    }

    @Override public int intValue() { return (int) value; }
    @Override public long longValue() { return value; }
    @Override public float floatValue() { return (float) value; }
    @Override public double doubleValue() { return (double) value; }
    public byte byteValue() { return (byte) value; }
    public short shortValue() { return (short) value; }

    @Override
    public int hashCode() {
        return hashCode(value);
    }

    public static int hashCode(long value) {
        return (int)(value ^ (value >>> 32));
    }

    @Override
    public boolean equals(Object obj) {
        if (obj instanceof Long other) {
            return value == other.value;
        }
        return false;
    }

    @Override
    public int compareTo(Long anotherLong) {
        return compare(this.value, anotherLong.value);
    }

    public static int compare(long x, long y) {
        return (x < y) ? -1 : ((x == y) ? 0 : 1);
    }

    public static int compareUnsigned(long x, long y) {
        return compare(x + MIN_VALUE, y + MIN_VALUE);
    }

    public static long divideUnsigned(long dividend, long divisor) {
        if (divisor < 0L) {
            // divisor is >= 2^63 unsigned
            return (compareUnsigned(dividend, divisor) < 0) ? 0L : 1L;
        }
        if (dividend >= 0) {
            return dividend / divisor;
        }
        // dividend is negative (>= 2^63 unsigned)
        // Split into two halvess to avoid overflow
        long q = ((dividend >>> 1) / divisor) << 1;
        long r = dividend - q * divisor;
        if (compareUnsigned(r, divisor) >= 0) {
            q++;
        }
        return q;
    }

    public static long remainderUnsigned(long dividend, long divisor) {
        return dividend - divideUnsigned(dividend, divisor) * divisor;
    }

    public static long highestOneBit(long i) {
        i |= (i >>  1);
        i |= (i >>  2);
        i |= (i >>  4);
        i |= (i >>  8);
        i |= (i >> 16);
        i |= (i >> 32);
        return i - (i >>> 1);
    }

    public static long lowestOneBit(long i) {
        return i & -i;
    }

    public static int numberOfLeadingZeros(long i) {
        if (i == 0) return 64;
        int n = 1;
        int x = (int)(i >>> 32);
        if (x == 0) { n += 32; x = (int)i; }
        if (x >>> 16 == 0) { n += 16; x <<= 16; }
        if (x >>> 24 == 0) { n +=  8; x <<=  8; }
        if (x >>> 28 == 0) { n +=  4; x <<=  4; }
        if (x >>> 30 == 0) { n +=  2; x <<=  2; }
        n -= (x >>> 31);
        return n;
    }

    public static int numberOfTrailingZeros(long i) {
        if (i == 0) return 64;
        int n = 63;
        long y;
        y = i << 32; if (y != 0) { n -= 32; i = y; }
        y = i << 16; if (y != 0) { n -= 16; i = y; }
        y = i <<  8; if (y != 0) { n -=  8; i = y; }
        y = i <<  4; if (y != 0) { n -=  4; i = y; }
        y = i <<  2; if (y != 0) { n -=  2; i = y; }
        return (int)(n - ((i << 1) >>> 63));
    }

    public static int bitCount(long i) {
        i = i - ((i >>> 1) & 0x5555555555555555L);
        i = (i & 0x3333333333333333L) + ((i >>> 2) & 0x3333333333333333L);
        i = (i + (i >>> 4)) & 0x0f0f0f0f0f0f0f0fL;
        i = i + (i >>> 8);
        i = i + (i >>> 16);
        i = i + (i >>> 32);
        return (int)i & 0x7f;
    }

    public static long rotateLeft(long i, int distance) {
        return (i << distance) | (i >>> -distance);
    }

    public static long rotateRight(long i, int distance) {
        return (i >>> distance) | (i << -distance);
    }

    public static long reverse(long i) {
        i = (i & 0x5555555555555555L) << 1 | (i >>> 1) & 0x5555555555555555L;
        i = (i & 0x3333333333333333L) << 2 | (i >>> 2) & 0x3333333333333333L;
        i = (i & 0x0f0f0f0f0f0f0f0fL) << 4 | (i >>> 4) & 0x0f0f0f0f0f0f0f0fL;
        return reverseBytes(i);
    }

    public static int signum(long i) {
        return (int) ((i >> 63) | (-i >>> 63));
    }

    public static long reverseBytes(long i) {
        i = (i & 0x00ff00ff00ff00ffL) << 8 | (i >>> 8) & 0x00ff00ff00ff00ffL;
        return (i << 48) | ((i & 0xffff0000L) << 16) |
               ((i >>> 16) & 0xffff0000L) | (i >>> 48);
    }

    public static long sum(long a, long b) { return a + b; }
    public static long max(long a, long b) { return (a >= b) ? a : b; }
    public static long min(long a, long b) { return (a <= b) ? a : b; }

    @Override
    public String toString() {
        return toString(value);
    }

    public static String toString(long i) {
        if (i == 0) return "0";
        if (i == MIN_VALUE) return "-9223372036854775808";
        boolean negative = (i < 0);
        if (negative) i = -i;
        // max digits for long is 19
        char[] buf = new char[20];
        int pos = 19;
        while (i > 0) {
            buf[pos--] = (char)('0' + (int)(i % 10));
            i = i / 10;
        }
        if (negative) {
            buf[pos--] = '-';
        }
        return new String(buf, pos + 1, 19 - pos);
    }

    public static String toString(long i, int radix) {
        if (radix < 2 || radix > 36) radix = 10;
        if (radix == 10) return toString(i);
        if (i == 0) return "0";
        boolean negative = (i < 0);
        // For negative non-decimal, handle MIN_VALUE specially
        char[] buf = new char[65]; // max binary is 64 digits + sign
        int pos = 64;
        if (negative) {
            if (i == MIN_VALUE) {
                // Handle MIN_VALUE: can't negate
                // Use unsigned conversion for the magnitude
                String posStr = toUnsignedString(-1L - (-1L - i), radix);
                // Actually, simpler: just handle digit-by-digit with negative remainders
                // We'll use a different approach
                long q;
                int r;
                while (i != 0) {
                    q = i / radix;
                    r = (int)(i - q * radix);
                    if (r > 0) {
                        // When dividing negative by positive, remainder can be positive in Java
                        // Actually in Java, remainder has sign of dividend
                        q++;
                        r -= radix;
                    }
                    buf[pos--] = Character.forDigit(-r, radix);
                    i = q;
                }
                buf[pos--] = '-';
                return new String(buf, pos + 1, 64 - pos);
            }
            i = -i;
        }
        while (i > 0) {
            buf[pos--] = Character.forDigit((int)(i % radix), radix);
            i = i / radix;
        }
        if (negative) {
            buf[pos--] = '-';
        }
        return new String(buf, pos + 1, 64 - pos);
    }

    public static String toHexString(long i) {
        return toUnsignedString(i, 16);
    }

    public static String toOctalString(long i) {
        return toUnsignedString(i, 8);
    }

    public static String toBinaryString(long i) {
        return toUnsignedString(i, 2);
    }

    public static String toUnsignedString(long i) {
        return toUnsignedString(i, 10);
    }

    public static String toUnsignedString(long i, int radix) {
        if (radix < 2 || radix > 36) radix = 10;
        if (i == 0) return "0";
        if (i > 0) return toString(i, radix);
        // Negative value means upper bit set — treat as unsigned
        // Use divideUnsigned / remainderUnsigned
        char[] buf = new char[65];
        int pos = 64;
        while (i != 0) {
            long q = divideUnsigned(i, (long) radix);
            int r = (int)(i - q * (long) radix);
            // r might be negative due to overflow; use remainderUnsigned
            if (r < 0) {
                r = (int) remainderUnsigned(i, (long) radix);
                q = divideUnsigned(i, (long) radix);
            }
            buf[pos--] = Character.forDigit(r, radix);
            i = q;
        }
        return new String(buf, pos + 1, 64 - pos);
    }
}
