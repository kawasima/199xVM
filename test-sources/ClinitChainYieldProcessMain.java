public class ClinitChainYieldProcessMain {
    private static class Base {
        static final String VALUE;

        static {
            long sum = 0L;
            for (int i = 0; i < 50_000; i++) {
                sum += i;
            }
            System.out.print("B");
            VALUE = sum == 0L ? "bad" : "base";
        }
    }

    private static class Mid extends Base {
        static final String VALUE;

        static {
            long sum = 0L;
            for (int i = 0; i < 50_000; i++) {
                sum += i;
            }
            System.out.print("M");
            VALUE = Base.VALUE + ":mid";
        }
    }

    private static class Leaf extends Mid {
        static final String VALUE;

        static {
            long sum = 0L;
            for (int i = 0; i < 50_000; i++) {
                sum += i;
            }
            System.out.print("L");
            VALUE = Mid.VALUE + ":leaf";
        }
    }

    public static void main(String[] args) {
        System.out.print(Leaf.VALUE);
    }
}
