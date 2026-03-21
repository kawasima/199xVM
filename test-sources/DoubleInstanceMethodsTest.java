public class DoubleInstanceMethodsTest {
    public static String run() {
        boolean doubleNan = Double.valueOf(Double.NaN).isNaN();
        boolean doubleInfinite = Double.valueOf(Double.POSITIVE_INFINITY).isInfinite();
        boolean floatNan = Float.valueOf(Float.NaN).isNaN();
        boolean floatInfinite = Float.valueOf(Float.NEGATIVE_INFINITY).isInfinite();
        return doubleNan + "|" + doubleInfinite + "|" + floatNan + "|" + floatInfinite;
    }
}
