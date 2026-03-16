import java.util.stream.IntStream;

public class IntStreamTest {
    public static String run() {
        int sum = IntStream.range(1, 11).filter(i -> i % 2 == 0).sum();
        return "sum=" + sum;
    }
}
