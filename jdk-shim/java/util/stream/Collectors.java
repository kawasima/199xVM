package java.util.stream;

import java.util.ArrayList;
import java.util.List;

public final class Collectors {
    private Collectors() {}

    public static Collector<CharSequence, StringBuilder, String> joining(
            CharSequence delimiter, CharSequence prefix, CharSequence suffix) {
        return new JoiningCollector(delimiter.toString(), prefix.toString(), suffix.toString());
    }

    public static Collector<CharSequence, StringBuilder, String> joining(CharSequence delimiter) {
        return new JoiningCollector(delimiter.toString(), "", "");
    }

    public static Collector<CharSequence, StringBuilder, String> joining() {
        return new JoiningCollector("", "", "");
    }

    public static <T> Collector<T, List<T>, List<T>> toList() {
        return new ToListCollector<>();
    }

    private static class JoiningCollector implements Collector<CharSequence, StringBuilder, String> {
        private final String delimiter;
        private final String prefix;
        private final String suffix;

        JoiningCollector(String delimiter, String prefix, String suffix) {
            this.delimiter = delimiter;
            this.prefix = prefix;
            this.suffix = suffix;
        }

        @Override
        public StringBuilder supplier() {
            return new StringBuilder();
        }

        @Override
        public void accumulator(StringBuilder sb, CharSequence element) {
            if (sb.length() > 0) sb.append(delimiter);
            sb.append(element);
        }

        @Override
        public String finisher(StringBuilder sb) {
            return prefix + sb.toString() + suffix;
        }
    }

    private static class ToListCollector<T> implements Collector<T, List<T>, List<T>> {
        @Override
        public List<T> supplier() {
            return new ArrayList<>();
        }

        @Override
        public void accumulator(List<T> list, T element) {
            list.add(element);
        }

        @Override
        public List<T> finisher(List<T> list) {
            return list;
        }
    }
}
