public class InterfaceDefaultMethodTest {
    interface Describable {
        String name();
        default String describe() {
            return "I am " + name();
        }
    }

    static class Thing implements Describable {
        public String name() { return "Thing"; }
        // describe() is not overridden — default method must be dispatched.
    }

    public static String run() {
        Describable d = new Thing();
        return d.describe();   // invokeinterface → default method dispatch
    }
}
