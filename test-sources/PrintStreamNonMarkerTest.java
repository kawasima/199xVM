import java.io.ByteArrayOutputStream;
import java.io.PrintStream;

public class PrintStreamNonMarkerTest {
    public static String run() {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        PrintStream ps = new PrintStream(out, true);
        ps.print("A");
        ps.println("B");
        ps.close();
        ps.print("C");
        return out.toString().replace("\n", "\\n") + "|" + ps.checkError();
    }
}
