import java.io.File;
import java.io.FileWriter;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileAttribute;
import java.util.Properties;

public class ClojureBootstrapShimsTest {
    public static String run() throws Exception {
        Properties properties = new Properties();
        properties.setProperty("alpha", "1");
        properties.setProperty("beta", "2");
        boolean propsOk = properties.stringPropertyNames().contains("alpha")
                && properties.stringPropertyNames().contains("beta");

        Path path = Files.createTempFile("clojure-", ".edn", new FileAttribute[0]);
        File file = path.toFile();
        FileWriter writer = new FileWriter(file);
        writer.close();
        boolean fileOk = file.getPath().startsWith("/tmp/clojure-") && file.getPath().endsWith(".edn");

        InetAddress address = InetAddress.getByName(null);
        ServerSocket server = new ServerSocket(4321, 0, address);
        String net = server.getLocalPort()
                + "|" + server.getInetAddress().getHostAddress()
                + "|" + server.isClosed();
        server.close();
        net += "|" + server.isClosed();

        return propsOk + "|" + fileOk + "|" + net;
    }
}
