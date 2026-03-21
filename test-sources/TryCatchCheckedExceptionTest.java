public class TryCatchCheckedExceptionTest {
    public static String run() {
        try {
            new java.io.FileReader("CAFEBABEx0/idonotexist");
            return "FAIL";
        } catch (Throwable t) {
            return t.getClass().getName();
        }
    }
}
