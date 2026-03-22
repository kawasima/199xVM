public class InheritedStaticFieldTest {
    static class Base {
        static int counter = 1;
    }

    static class Child extends Base {
    }

    public static String run() {
        int before = Child.counter;
        Child.counter = 7;
        return before + "|" + Base.counter + "|" + Child.counter;
    }
}
