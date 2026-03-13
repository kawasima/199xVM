import java.net.URL;
import java.net.URI;

public class URLTest {
    public static String run() throws Exception {
        URL url = new URL("https://example.com:8080/path?q=1#frag");
        String parts = url.getProtocol() + "|"
                + url.getHost() + "|"
                + url.getPort() + "|"
                + url.getPath() + "|"
                + url.getQuery() + "|"
                + url.getRef();

        // URI.toURL() round-trip
        URI uri = new URI("https://example.com/foo");
        URL fromUri = uri.toURL();

        return parts + "|" + fromUri.getHost();
    }
}
