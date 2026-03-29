public class StaticAccessInitTest {
    static class Holder {
        static int initCount = 0;
        static int value = initValue();

        static int initValue() {
            initCount++;
            return 41;
        }

        static int read() {
            return value;
        }
    }

    public static String run() {
        int first = Holder.value;
        Holder.value = first + 1;
        int second = Holder.read();
        return first + "|" + second + "|" + Holder.initCount;
    }
}
