/**
 * Tests that default methods on functional interfaces work correctly when
 * invoked on lambda instances (BytecodeLambda objects).
 */
@FunctionalInterface
interface Mapper<I, O> {
    O apply(I in, String ctx);

    default O apply(I in) {
        return apply(in, "default");
    }
}

public class LambdaDefaultMethodTest {
    public static String run() {
        // Lambda implementing SAM; call goes through default method
        Mapper<String, String> m = (in, ctx) -> in + ":" + ctx;
        return m.apply("hello");
    }
}
