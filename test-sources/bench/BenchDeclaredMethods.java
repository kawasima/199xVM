public class BenchDeclaredMethods {
    static final class Sample {
        public int add(int x, int y) {
            return x + y;
        }

        public String label() {
            return "sample";
        }

        private static long widen(int x) {
            return x;
        }
    }

    public static int run() {
        int sum = 0;
        for (int i = 0; i < 1000; i++) {
            sum += Sample.class.getDeclaredMethods().length;
        }
        return sum;
    }
}
