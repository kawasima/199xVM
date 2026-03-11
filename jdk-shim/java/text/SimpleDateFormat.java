package java.text;

import java.util.Date;

public class SimpleDateFormat extends DateFormat {
    private String pattern;

    public SimpleDateFormat() {
        this.pattern = "";
    }

    public SimpleDateFormat(String pattern) {
        this.pattern = (pattern == null) ? "" : pattern;
    }

    public String toPattern() {
        return pattern;
    }

    public void applyPattern(String pattern) {
        this.pattern = (pattern == null) ? "" : pattern;
    }

    @Override
    public String format(Date date) {
        return super.format(date);
    }
}
