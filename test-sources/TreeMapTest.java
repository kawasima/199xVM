import java.util.TreeMap;
import java.util.Map;

public class TreeMapTest {
    public static String run() {
        TreeMap<String, Integer> map = new TreeMap<>();
        map.put("cherry", 3);
        map.put("apple", 1);
        map.put("banana", 2);
        StringBuilder sb = new StringBuilder();
        for (Map.Entry<String, Integer> entry : map.entrySet()) {
            if (sb.length() > 0) sb.append(",");
            sb.append(entry.getKey());
            sb.append("=");
            sb.append(entry.getValue());
        }
        return sb.toString();
    }
}
