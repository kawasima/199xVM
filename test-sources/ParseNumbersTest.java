public class ParseNumbersTest {
    public static String run() {
        double d = Double.parseDouble("3.5");
        float f = Float.parseFloat("2.25");
        double inf = Double.parseDouble("Infinity");
        float nan = Float.parseFloat("NaN");
        boolean bad;
        try {
            Double.parseDouble("bad");
            bad = false;
        } catch (NumberFormatException e) {
            bad = true;
        }
        return d + "|" + f + "|" + Double.isInfinite(inf) + "|" + Float.isNaN(nan) + "|" + bad;
    }
}
