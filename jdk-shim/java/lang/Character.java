package java.lang;

import java.io.Serializable;

public final class Character implements Serializable, Comparable<Character> {
    public static final int MIN_RADIX = 2;
    public static final int MAX_RADIX = 36;
    private final char value;
    public Character(char value) { this.value = value; }
    public static Character valueOf(char c) { return new Character(c); }
    public char charValue() { return value; }
    @Override public int compareTo(Character another) { return value - another.value; }
    @Override public String toString() { return String.valueOf(value); }

    public static int digit(char ch, int radix) {
        int val;
        if (ch >= '0' && ch <= '9') val = ch - '0';
        else if (ch >= 'a' && ch <= 'z') val = ch - 'a' + 10;
        else if (ch >= 'A' && ch <= 'Z') val = ch - 'A' + 10;
        else return -1;
        return (val < radix) ? val : -1;
    }

    public static char forDigit(int digit, int radix) {
        if (digit < 0 || digit >= radix || radix < 2 || radix > 36) return '\0';
        if (digit < 10) return (char) ('0' + digit);
        return (char) ('a' + digit - 10);
    }

    public static boolean isDigit(char ch) {
        return ch >= '0' && ch <= '9';
    }

    public static boolean isLetter(char ch) {
        return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z');
    }

    public static boolean isLetterOrDigit(char ch) {
        return isLetter(ch) || isDigit(ch);
    }

    public static boolean isUpperCase(char ch) {
        return ch >= 'A' && ch <= 'Z';
    }

    public static boolean isLowerCase(char ch) {
        return ch >= 'a' && ch <= 'z';
    }

    public static char toUpperCase(char ch) {
        return isLowerCase(ch) ? (char)(ch - 32) : ch;
    }

    public static char toLowerCase(char ch) {
        return isUpperCase(ch) ? (char)(ch + 32) : ch;
    }

    public static boolean isWhitespace(char ch) {
        return ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' || ch == '\f';
    }

    public static boolean isJavaIdentifierStart(char ch) {
        return isLetter(ch) || ch == '_' || ch == '$';
    }

    public static boolean isJavaIdentifierPart(char ch) {
        return isJavaIdentifierStart(ch) || isDigit(ch);
    }
}
