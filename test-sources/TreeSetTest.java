import java.util.TreeSet;

public class TreeSetTest {
    public static String run() {
        TreeSet<Integer> set = new TreeSet<>();
        set.add(30);
        set.add(10);
        set.add(20);
        StringBuilder sb = new StringBuilder();
        for (Integer i : set) {
            if (sb.length() > 0) sb.append(",");
            sb.append(i);
        }
        return sb.toString();
    }
}
