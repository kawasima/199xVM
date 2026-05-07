public class DefinedClassReflectionMetadataTest {
    public static String run() {
        LoaderPhase0Fixtures.ExposedLoader loader = new LoaderPhase0Fixtures.ExposedLoader();
        Class<?> klass = loader.define(
                "phase0.Isolated",
                LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.ISOLATED_B64));
        String visibility = ((klass.getModifiers() & 1) != 0) ? "public" : "not-public";
        boolean declaringClassesMatch =
                klass.getDeclaredFields()[0].getDeclaringClass() == klass
                && klass.getDeclaredMethods()[0].getDeclaringClass() == klass
                && klass.getDeclaredConstructors()[0].getDeclaringClass() == klass;
        return visibility
                + "|fields=" + klass.getDeclaredFields().length
                + "|methods=" + klass.getDeclaredMethods().length
                + "|ctors=" + klass.getDeclaredConstructors().length
                + "|annotations=" + klass.getDeclaredAnnotations().length
                + "|declaring=" + (declaringClassesMatch ? "same" : "different");
    }
}
