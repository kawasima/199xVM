public class ClassLoaderNegativeTest {
    public static String run() {
        // Class.forName for a non-existent class must throw ClassNotFoundException.
        try {
            Class.forName("com.example.NonExistentClass");
            return "no-exception";
        } catch (ClassNotFoundException e) {
            return "ClassNotFoundException:" + e.getMessage();
        }
    }
}
