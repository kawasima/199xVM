public class AbstractMethodTest {
    interface Greeter {
        String greet();
    }

    static class ConcreteGreeter implements Greeter {
        public String greet() { return "hello"; }
    }

    public static String run() {
        // Concrete implementation — must NOT throw AbstractMethodError
        Greeter g = new ConcreteGreeter();
        return g.greet();
    }
}
