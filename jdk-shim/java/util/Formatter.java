package java.util;

public class Formatter {
    private final StringBuilder sb;

    public Formatter() {
        this.sb = new StringBuilder();
    }

    public Formatter format(String fmt, Object... args) {
        int argIndex = 0;
        int i = 0;
        while (i < fmt.length()) {
            char c = fmt.charAt(i);
            if (c == '%' && i + 1 < fmt.length()) {
                char spec = fmt.charAt(i + 1);
                if (spec == 's' || spec == 'd' || spec == 'f') {
                    if (argIndex < args.length) {
                        Object arg = args[argIndex++];
                        sb.append(arg == null ? "null" : arg.toString());
                    }
                    i += 2;
                } else if (spec == '%') {
                    sb.append('%');
                    i += 2;
                } else if (spec == 'n') {
                    sb.append('\n');
                    i += 2;
                } else {
                    sb.append(c);
                    i++;
                }
            } else {
                sb.append(c);
                i++;
            }
        }
        return this;
    }

    @Override
    public String toString() {
        return sb.toString();
    }
}
