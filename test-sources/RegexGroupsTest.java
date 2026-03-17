import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class RegexGroupsTest {
    public static String run() {
        Matcher m = Pattern.compile("(\\d+)\\.(\\d+)\\.(\\d+)(?:-([a-zA-Z0-9_]+))?(?:-(SNAPSHOT))?")
            .matcher("1.12.0");
        boolean matches = m.matches();
        return matches
            + "|gc=" + m.groupCount()
            + "|g0=" + m.group()
            + "|g1=" + m.group(1)
            + "|g2=" + m.group(2)
            + "|g3=" + m.group(3)
            + "|g4=" + m.group(4)
            + "|g5=" + m.group(5);
    }
}
