import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class RegexFindEscapedParensTest {
    public static String run() {
        String input = "Wrong number of args (0) passed to: :kw";

        Matcher exact = Pattern.compile("Wrong number of args \\(0\\) passed to: :kw").matcher(input);
        boolean found = exact.find();
        String group = found ? exact.group(0) : "miss";

        boolean miss = Pattern.compile("Wrong number of args \\(1\\) passed to: :kw")
            .matcher(input)
            .find();

        return found + "|" + group + "|" + miss;
    }
}
