public class TryCatchTest {
    public static String run() {
        String result;
        try {
            Object obj = null;
            obj.toString(); // should throw NullPointerException
            result = "FAIL";
        } catch (NullPointerException e) {
            result = "CAUGHT";
        }
        return result;
    }
}
