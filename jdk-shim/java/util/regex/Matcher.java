package java.util.regex;

public final class Matcher {
    private final Pattern pattern;
    private String input;
    private int searchIndex;
    private int matchStart = -1;
    private int matchEnd = -1;

    Matcher(Pattern pattern, String input) {
        this.pattern = pattern;
        this.input = (input == null) ? "" : input;
        this.searchIndex = 0;
    }

    public Pattern pattern() {
        return pattern;
    }

    public Matcher reset() {
        this.searchIndex = 0;
        this.matchStart = -1;
        this.matchEnd = -1;
        return this;
    }

    public Matcher reset(CharSequence input) {
        this.input = (input == null) ? "" : input.toString();
        return reset();
    }

    public boolean matches() {
        String r = pattern.pattern();
        if (".*".equals(r)) {
            matchStart = 0;
            matchEnd = input.length();
            return true;
        }
        if (r.equals(input)) {
            matchStart = 0;
            matchEnd = input.length();
            return true;
        }
        matchStart = -1;
        matchEnd = -1;
        return false;
    }

    public boolean find() {
        String r = pattern.pattern();
        if (r.length() == 0) {
            matchStart = -1;
            matchEnd = -1;
            return false;
        }
        int at = -1;
        if (searchIndex <= input.length()) {
            int rel = input.substring(searchIndex).indexOf(r);
            if (rel >= 0) {
                at = searchIndex + rel;
            }
        }
        if (at < 0) {
            matchStart = -1;
            matchEnd = -1;
            return false;
        }
        matchStart = at;
        matchEnd = at + r.length();
        searchIndex = matchEnd;
        return true;
    }

    public int start() {
        return matchStart;
    }

    public int end() {
        return matchEnd;
    }

    public String group() {
        if (matchStart < 0 || matchEnd < 0) {
            return null;
        }
        return input.substring(matchStart, matchEnd);
    }

    public String group(int group) {
        return group == 0 ? group() : null;
    }

    public String group(String name) {
        return group();
    }

    public String replaceAll(String replacement) {
        String r = pattern.pattern();
        if (r.length() == 0) {
            return input;
        }
        String rep = replacement == null ? "" : replacement;
        StringBuilder sb = new StringBuilder();
        int from = 0;
        int at;
        while (from <= input.length()) {
            int rel = input.substring(from).indexOf(r);
            if (rel < 0) {
                break;
            }
            at = from + rel;
            sb.append(input.substring(from, at));
            sb.append(rep);
            from = at + r.length();
        }
        sb.append(input.substring(from));
        return sb.toString();
    }
}
