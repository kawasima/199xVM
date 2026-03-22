public class IntegerBitIntrinsicsTest {
    public static String run() {
        int i = 0x55aa55aa;
        long l = 0x55aa55aa55aa55aaL;
        return Integer.bitCount(i)
            + "|"
            + Integer.numberOfLeadingZeros(0x10)
            + "|"
            + Integer.numberOfTrailingZeros(0x10)
            + "|"
            + Integer.rotateLeft(1, 5)
            + "|"
            + Integer.rotateRight(32, 5)
            + "|"
            + Integer.reverseBytes(0x01020304)
            + "|"
            + Long.bitCount(l)
            + "|"
            + Long.numberOfLeadingZeros(0x10L)
            + "|"
            + Long.numberOfTrailingZeros(0x10L)
            + "|"
            + Long.rotateLeft(1L, 8)
            + "|"
            + Long.rotateRight(256L, 8)
            + "|"
            + Long.reverseBytes(0x0102030405060708L);
    }
}
