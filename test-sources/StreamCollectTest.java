import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;

public class StreamCollectTest {
    public static String run() {
        List<String> list = new ArrayList<>();
        list.add("apple");
        list.add("banana");
        list.add("avocado");
        list.add("cherry");
        // Filter strings starting with 'a', convert to uppercase, collect
        List<String> result = list.stream()
            .filter(s -> s.startsWith("a"))
            .map(s -> s.toUpperCase())
            .collect(Collectors.toList());
        return String.join(",", result);
    }
}
