package java.lang;

public final class Math {
    public static final double E = 2.7182818284590452354;
    public static final double PI = 3.14159265358979323846;

    private Math() {}

    public static int abs(int a) { return a < 0 ? -a : a; }
    public static long abs(long a) { return a < 0 ? -a : a; }
    public static float abs(float a) { return a <= 0.0f ? 0.0f - a : a; }
    public static double abs(double a) { return a <= 0.0d ? 0.0d - a : a; }

    public static int min(int a, int b) { return a <= b ? a : b; }
    public static long min(long a, long b) { return a <= b ? a : b; }
    public static float min(float a, float b) { return (a <= b) ? a : b; }
    public static double min(double a, double b) { return (a <= b) ? a : b; }

    public static int max(int a, int b) { return a >= b ? a : b; }
    public static long max(long a, long b) { return a >= b ? a : b; }
    public static float max(float a, float b) { return (a >= b) ? a : b; }
    public static double max(double a, double b) { return (a >= b) ? a : b; }

    public static int round(float a) { return (int) (a + (a >= 0.0f ? 0.5f : -0.5f)); }
    public static long round(double a) { return (long) (a + (a >= 0.0d ? 0.5d : -0.5d)); }

    public static double floor(double a) {
        long i = (long) a;
        return (a < i) ? (double) (i - 1) : (double) i;
    }

    public static double ceil(double a) {
        long i = (long) a;
        return (a > i) ? (double) (i + 1) : (double) i;
    }

    public static int addExact(int x, int y) {
        int r = x + y;
        if (((x ^ r) & (y ^ r)) < 0) throw new ArithmeticException("integer overflow");
        return r;
    }

    public static long addExact(long x, long y) {
        long r = x + y;
        if (((x ^ r) & (y ^ r)) < 0) throw new ArithmeticException("long overflow");
        return r;
    }

    public static double sqrt(double x) {
        if (x < 0.0 || x != x) return 0.0d / 0.0d;
        if (x == 0.0 || x == 1.0) return x;
        double g = x > 1.0 ? x : 1.0;
        for (int i = 0; i < 30; i++) {
            g = 0.5d * (g + x / g);
        }
        return g;
    }

    public static double log(double x) {
        if (x < 0.0 || x != x) return 0.0d / 0.0d;
        if (x == 0.0) return -1.0d / 0.0d;
        if (x == 1.0) return 0.0d;
        if (x == 1.0d / 0.0d) return x;

        int k = 0;
        while (x > 2.0) {
            x *= 0.5;
            k++;
        }
        while (x < 1.0) {
            x *= 2.0;
            k--;
        }
        double y = (x - 1.0) / (x + 1.0);
        double y2 = y * y;
        double sum = 0.0;
        double term = y;
        for (int n = 1; n <= 39; n += 2) {
            sum += term / n;
            term *= y2;
        }
        return 2.0 * sum + k * 0.6931471805599453d;
    }

    public static float scalb(float f, int scaleFactor) {
        return (float) scalb((double) f, scaleFactor);
    }

    public static double scalb(double d, int scaleFactor) {
        if (d == 0.0 || d != d || d == 1.0d / 0.0d || d == -1.0d / 0.0d) return d;
        if (scaleFactor > 0) {
            for (int i = 0; i < scaleFactor; i++) d *= 2.0d;
        } else if (scaleFactor < 0) {
            for (int i = 0; i > scaleFactor; i--) d *= 0.5d;
        }
        return d;
    }

    public static long clamp(long value, long min, long max) {
        if (value < min) return min;
        if (value > max) return max;
        return value;
    }
}
