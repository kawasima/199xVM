public class ClassGetClassLoaderTest {
    public static String run() {
        StringBuilder sb = new StringBuilder();

        sb.append(Integer.TYPE.getClassLoader() == null ? "primitive:null" : "primitive:loader");
        sb.append("|");

        LoaderPhase0Fixtures.ExposedLoader loader = new LoaderPhase0Fixtures.ExposedLoader();
        byte[] bytes = LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.SAME_NAME_B64);
        Class<?> defined = loader.define("phase0.SameName", bytes);
        sb.append(defined.getClassLoader() == loader ? "custom:same" : "custom:other");

        return sb.toString();
    }
}
