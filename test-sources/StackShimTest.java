import java.util.Stack;

public class StackShimTest {
    public static String run() {
        Stack<String> stack = new Stack<>();
        String first = stack.push("alpha");
        String second = stack.push("beta");
        String peek = stack.peek();
        int searchAlpha = stack.search("alpha");
        String pop = stack.pop();
        boolean emptyAfterPop = stack.empty();
        String last = stack.pop();
        boolean emptyAtEnd = stack.empty();
        return first + "|" + second + "|" + peek + "|" + searchAlpha + "|" + pop + "|" + emptyAfterPop + "|" + last + "|" + emptyAtEnd;
    }
}
