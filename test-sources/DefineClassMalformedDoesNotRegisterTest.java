public class DefineClassMalformedDoesNotRegisterTest {
    public static String run() {
        LoaderPhase0Fixtures.ExposedLoader loader = new LoaderPhase0Fixtures.ExposedLoader();
        byte[] bytes = LoaderPhase0Fixtures.bytesWithInvalidSuperClass(
                LoaderPhase0Fixtures.SAME_NAME_B64);
        String first = defineFailure(loader, bytes);
        String second = defineFailure(loader, bytes);
        return first + "|" + second;
    }

    private static String defineFailure(LoaderPhase0Fixtures.ExposedLoader loader, byte[] bytes) {
        try {
            loader.define("phase0.SameName", bytes);
            return "no-error";
        } catch (ClassFormatError e) {
            return "ClassFormatError";
        } catch (LinkageError e) {
            return "LinkageError";
        } catch (Throwable t) {
            return LoaderPhase0Fixtures.simpleName(t);
        }
    }
}
