public class RepeatedForNameProfileTest {
    static class Target {
        static int initCount = 0;

        static {
            initCount++;
        }
    }

    public static String run() throws Exception {
        Class<?> first = null;
        boolean same = true;
        for (int i = 0; i < 8; i++) {
            Class<?> current = Class.forName("RepeatedForNameProfileTest$Target");
            if (first == null) {
                first = current;
            } else {
                same &= first == current;
            }
        }
        return same + "|" + Target.initCount;
    }
}
