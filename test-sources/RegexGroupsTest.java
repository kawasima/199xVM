import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class RegexGroupsTest {
    public static String run() {
        Pattern p = Pattern.compile("(\\d+)\\.(\\d+)\\.(\\d+)");
        Matcher m = p.matcher("1.12.0");
        if (!m.matches()) return "no-match";
        int gc = m.groupCount();
        String g0 = m.group(0);
        String g1 = m.group(1);
        String g2 = m.group(2);
        String g3 = m.group(3);
        return gc + "|" + g0 + "|" + g1 + "|" + g2 + "|" + g3;
    }
}
