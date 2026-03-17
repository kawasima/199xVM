public class StringReplaceTest {
    public static String run() {
        String s = "clojure/core_proxy__init";
        String result = s.replace('/', '.');
        return result;
    }
}
