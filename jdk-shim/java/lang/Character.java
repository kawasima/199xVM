package java.lang;

import java.io.Serializable;

public final class Character implements Serializable, Comparable<Character> {
    private final char value;
    public Character(char value) { this.value = value; }
    public static Character valueOf(char c) { return new Character(c); }
    public char charValue() { return value; }
    @Override public int compareTo(Character another) { return value - another.value; }
    @Override public String toString() { return String.valueOf(value); }

    public static int digit(char ch, int radix) {
        if (radix == 10) {
            if (ch >= '0' && ch <= '9') return ch - '0';
            return -1;
        }
        return -1;
    }
}
