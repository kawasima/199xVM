import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Random;

public class RandomShimTest {
    private static final char[] HEX = "0123456789abcdef".toCharArray();

    private static String hex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            int value = b & 0xff;
            sb.append(HEX[value >>> 4]);
            sb.append(HEX[value & 0x0f]);
        }
        return sb.toString();
    }

    public static String run() {
        Random a = new Random(123L);
        Random b = new Random(123L);
        boolean sameInts = a.nextInt(1000) == b.nextInt(1000) && a.nextInt(1000) == b.nextInt(1000);

        ArrayList<Integer> left = new ArrayList<>(Arrays.asList(1, 2, 3, 4, 5));
        ArrayList<Integer> right = new ArrayList<>(Arrays.asList(1, 2, 3, 4, 5));
        Collections.shuffle(left, new Random(123L));
        Collections.shuffle(right, new Random(123L));
        boolean sameShuffle = left.equals(right);
        boolean changedOrder = !left.equals(Arrays.asList(1, 2, 3, 4, 5));

        SecureRandom secure = new SecureRandom();
        byte[] bytes1 = new byte[4];
        byte[] bytes2 = new byte[4];
        secure.nextBytes(bytes1);
        secure.nextBytes(bytes2);
        // CSPRNG: two calls should produce different results (extremely unlikely to collide)
        boolean secureNonZero = (bytes1[0] | bytes1[1] | bytes1[2] | bytes1[3]) != 0
                             || (bytes2[0] | bytes2[1] | bytes2[2] | bytes2[3]) != 0;
        boolean secureDifferent = !Arrays.equals(bytes1, bytes2);

        return sameInts + "|" + sameShuffle + "|" + changedOrder + "|" + left + "|" + secureNonZero + "|" + secureDifferent;
    }
}
