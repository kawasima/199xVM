/**
 * Tests that lambda SAM dispatch respects argument arity.
 * An interface with overloaded methods (1-arg and 2-arg) must dispatch
 * the correct overload, not match by name alone.
 */
public class LambdaOverloadTest {
    interface Processor {
        String process(String input, String suffix);
        default String process(String input) {
            return process(input, "!");
        }
    }

    public static String run() {
        // The lambda implements process(String, String).
        // Calling process(String) should invoke the default method,
        // which in turn calls process(String, String).
        Processor p = (a, b) -> a.toUpperCase() + b;
        return p.process("hello") + "|" + p.process("hi", "?");
    }
}
