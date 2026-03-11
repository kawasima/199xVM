package java.lang;

public interface CharSequence {
    int length();
    char charAt(int index);
    CharSequence subSequence(int start, int end);
    String toString();

    default void getChars(int srcBegin, int srcEnd, char[] dst, int dstBegin) {
        if (srcBegin < 0 || srcEnd < srcBegin || dstBegin < 0 || dstBegin + (srcEnd - srcBegin) > dst.length) {
            throw new IndexOutOfBoundsException();
        }
        for (int i = srcBegin; i < srcEnd; i++) {
            dst[dstBegin++] = charAt(i);
        }
    }
}
