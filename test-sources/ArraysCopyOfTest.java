import java.util.Arrays;

public class ArraysCopyOfTest {
    public static String run() {
        int[] src = {1, 2, 3};
        int[] dst = Arrays.copyOf(src, 5);
        // dst should be [1, 2, 3, 0, 0]
        return "" + dst.length + ":" + dst[0] + "," + dst[2] + "," + dst[4];
    }
}
