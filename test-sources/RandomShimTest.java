import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Random;
import java.security.SecureRandom;

public class RandomShimTest {
    public static String runSeededRandom() {
        Random a = new Random(123L);
        Random b = new Random(123L);
        boolean sameInts = a.nextInt(1000) == b.nextInt(1000) && a.nextInt(1000) == b.nextInt(1000);

        ArrayList<Integer> left = new ArrayList<>(Arrays.asList(1, 2, 3, 4, 5));
        ArrayList<Integer> right = new ArrayList<>(Arrays.asList(1, 2, 3, 4, 5));
        Collections.shuffle(left, new Random(123L));
        Collections.shuffle(right, new Random(123L));
        boolean sameShuffle = left.equals(right);
        boolean changedOrder = !left.equals(Arrays.asList(1, 2, 3, 4, 5));

        return sameInts + "|" + sameShuffle + "|" + changedOrder + "|" + left;
    }

    public static String runSecureRandomApi() {
        SecureRandom secure = new SecureRandom();
        byte[] bytes1 = new byte[4];
        secure.nextBytes(bytes1);
        byte[] seed = secure.generateSeed(4);
        try {
            SecureRandom strong = SecureRandom.getInstanceStrong();
            return (bytes1.length == 4) + "|" + (seed.length == 4) + "|" + (strong != null);
        } catch (Exception e) {
            return "false|false|false";
        }
    }
}
