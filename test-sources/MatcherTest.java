import java.util.regex.Pattern;

public class MatcherTest {
    public static String run() {
        StringBuilder sb = new StringBuilder();

        // 1. already-anchored pattern: ^foo$ must match "foo"
        sb.append(Pattern.compile("^foo$").matcher("foo").matches() ? "true" : "false");
        sb.append("|");

        // 2. already-anchored pattern: ^foo$ must NOT match "foobar"
        sb.append(Pattern.compile("^foo$").matcher("foobar").matches() ? "true" : "false");
        sb.append("|");

        // 3. email-like pattern (the original bug report)
        String emailRegex = "^[a-zA-Z0-9._%+\\-]{1,64}@[a-zA-Z0-9.\\-]{1,255}\\.[a-zA-Z]{2,}$";
        sb.append(Pattern.compile(emailRegex).matcher("alice@example.com").matches() ? "true" : "false");
        sb.append("|");

        // 4. escaped literal \$ at end must NOT be treated as anchor
        // Pattern "^foo\$" matches literal "foo$", not "foo"
        sb.append(Pattern.compile("^foo\\$").matcher("foo$").matches() ? "true" : "false");
        sb.append("|");

        // 5. non-anchored pattern still does full-string match via wrapping
        sb.append(Pattern.compile("foo").matcher("foo").matches() ? "true" : "false");
        sb.append("|");
        sb.append(Pattern.compile("foo").matcher("foobar").matches() ? "true" : "false");
        sb.append("|");

        // 6. alternation ^foo$|bar$ must NOT match "xxbar" (bar$ branch is not start-anchored)
        sb.append(Pattern.compile("^foo$|bar$").matcher("xxbar").matches() ? "true" : "false");

        return sb.toString();
    }
}
