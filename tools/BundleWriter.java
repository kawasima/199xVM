import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;

public final class BundleWriter {
    private static final byte[] RESOURCE_MAGIC = "RSRC".getBytes(StandardCharsets.US_ASCII);

    private BundleWriter() {}

    public static void main(String[] args) throws Exception {
        if (args.length < 3) {
            usage();
            System.exit(2);
        }

        Path output = Path.of(args[0]);
        List<Path> classRoots = new ArrayList<>();
        List<Path> resourceRoots = new ArrayList<>();

        for (int i = 1; i < args.length; i += 2) {
            if (i + 1 >= args.length) {
                usage();
                System.exit(2);
            }
            switch (args[i]) {
                case "--class-root" -> classRoots.add(Path.of(args[i + 1]));
                case "--resource-root" -> resourceRoots.add(Path.of(args[i + 1]));
                default -> {
                    usage();
                    System.exit(2);
                }
            }
        }

        if (classRoots.isEmpty() && resourceRoots.isEmpty()) {
            System.err.println("BundleWriter: at least one --class-root or --resource-root is required");
            System.exit(2);
        }

        if (output.getParent() != null) {
            Files.createDirectories(output.getParent());
        }

        int classCount = 0;
        int resourceCount = 0;
        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(output))) {
            for (Path root : classRoots) {
                for (Path classFile : sortedFiles(root, path -> path.toString().endsWith(".class"))) {
                    writeClassEntry(out, classFile);
                    classCount++;
                }
            }
            for (Path root : resourceRoots) {
                for (Path resourceFile : sortedFiles(root, path -> !path.toString().endsWith(".class"))) {
                    writeResourceEntry(out, root, resourceFile);
                    resourceCount++;
                }
            }
        }

        long totalBytes = Files.size(output);
        if (resourceCount == 0) {
            System.out.printf("Bundled %d classes -> %s (%d bytes)%n", classCount, output, totalBytes);
        } else {
            System.out.printf(
                "Bundled %d classes and %d resources -> %s (%d bytes)%n",
                classCount,
                resourceCount,
                output,
                totalBytes
            );
        }
    }

    private static void usage() {
        System.err.println(
            "usage: java tools/BundleWriter.java <out.bin> " +
            "[--class-root <dir> ...] [--resource-root <dir> ...]"
        );
    }

    private static List<Path> sortedFiles(Path root, java.util.function.Predicate<Path> include) throws IOException {
        if (!Files.isDirectory(root)) {
            throw new IOException("bundle root is not a directory: " + root);
        }
        try (Stream<Path> paths = Files.walk(root)) {
            return paths
                .filter(Files::isRegularFile)
                .filter(include)
                .sorted(Comparator.comparing(path -> normalizePath(root.relativize(path))))
                .toList();
        }
    }

    private static void writeClassEntry(OutputStream out, Path classFile) throws IOException {
        byte[] bytes = Files.readAllBytes(classFile);
        writeU32(out, bytes.length);
        out.write(bytes);
    }

    private static void writeResourceEntry(OutputStream out, Path root, Path resourceFile) throws IOException {
        byte[] pathBytes = normalizePath(root.relativize(resourceFile)).getBytes(StandardCharsets.UTF_8);
        byte[] data = Files.readAllBytes(resourceFile);
        long lastModified = Files.getLastModifiedTime(resourceFile).toMillis();
        long payloadLength = 4L + 4L + 8L + 4L + pathBytes.length + data.length;
        if (payloadLength > Integer.MAX_VALUE) {
            throw new IOException("resource entry too large: " + resourceFile);
        }
        writeU32(out, (int) payloadLength);
        out.write(RESOURCE_MAGIC);
        writeU32(out, pathBytes.length);
        writeU64(out, lastModified);
        writeU32(out, data.length);
        out.write(pathBytes);
        out.write(data);
    }

    private static void writeU32(OutputStream out, int value) throws IOException {
        out.write((value >>> 24) & 0xff);
        out.write((value >>> 16) & 0xff);
        out.write((value >>> 8) & 0xff);
        out.write(value & 0xff);
    }

    private static void writeU64(OutputStream out, long value) throws IOException {
        out.write((int) ((value >>> 56) & 0xff));
        out.write((int) ((value >>> 48) & 0xff));
        out.write((int) ((value >>> 40) & 0xff));
        out.write((int) ((value >>> 32) & 0xff));
        out.write((int) ((value >>> 24) & 0xff));
        out.write((int) ((value >>> 16) & 0xff));
        out.write((int) ((value >>> 8) & 0xff));
        out.write((int) (value & 0xff));
    }

    private static String normalizePath(Path relativePath) {
        return relativePath.toString().replace('\\', '/');
    }
}
