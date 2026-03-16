import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;

public class StreamJoinTest {
    public static String run() {
        List<String> list = new ArrayList<>();
        list.add("a");
        list.add("b");
        list.add("c");
        return list.stream().collect(Collectors.joining("-"));
    }
}
