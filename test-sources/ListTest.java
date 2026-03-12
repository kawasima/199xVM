import java.util.List;
import java.util.ArrayList;

public class ListTest {
    public static String run() {
        List<String> list = new ArrayList<>();
        list.add("hello");
        list.add("world");
        return list.size() + ": " + list.get(0) + " " + list.get(1);
    }
}
