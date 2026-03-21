public class ClinitYieldProcessMain {
    private static final class HeavyInit {
        static final String VALUE;

        static {
            long sum = 0L;
            for (int i = 0; i < 50_000; i++) {
                sum += i;
            }
            System.out.print("I");
            VALUE = sum == 0L ? "bad" : "done";
        }
    }

    public static void main(String[] args) {
        System.out.print(HeavyInit.VALUE);
    }
}
