import java.util.ArrayList;
import java.util.Optional;
import java.util.function.BinaryOperator;

public class StreamReduceTest {
    public static String run() {
        ArrayList<String> list = new ArrayList<>();
        list.add("a");
        list.add("b");
        list.add("c");
        Optional<String> result = list.stream().reduce((a, b) -> a + b);
        // Also test empty stream
        ArrayList<String> empty = new ArrayList<>();
        Optional<String> none = empty.stream().reduce((a, b) -> a + b);
        return result.get() + ":" + none.isPresent();
    }
}
