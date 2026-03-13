import java.net.URLEncoder;
import java.net.URLDecoder;
import java.net.URI;

public class NetTest {
    public static String run() throws Exception {
        String enc = URLEncoder.encode("hello world", "UTF-8");
        String dec = URLDecoder.decode(enc, "UTF-8");
        URI uri = new URI("https://example.com/path?q=1");
        return enc + "|" + dec + "|" + uri.getHost();
    }
}
