public class InvokespecialResolvedOwnerTest {
    public static String run() throws Exception {
        LoaderPhase0Fixtures.BytesLoader loader = new LoaderPhase0Fixtures.BytesLoader(
                new String[] { "phase0.SpecialChild", "phase0.SpecialBase" },
                new byte[][] {
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.SPECIAL_CHILD_B64),
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.SPECIAL_BASE_B64)
                });
        return LoaderPhase0Fixtures.invokeString(loader.loadClass("phase0.SpecialChild"), "run");
    }
}
