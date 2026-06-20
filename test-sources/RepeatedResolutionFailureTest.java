public class RepeatedResolutionFailureTest {
    public static String run() throws Exception {
        return new StringBuilder()
                .append(missingClass())
                .append('|')
                .append(missingField())
                .append('|')
                .append(missingMethod())
                .append('|')
                .append(missingInterfaceMethod())
                .toString();
    }

    public static String missingClass() throws Exception {
        return LoaderPhase0Fixtures.repeatedFailure(
                "phase0.MissingClassCaller",
                new String[] { "phase0.MissingClassCaller" },
                new byte[][] {
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.MISSING_CLASS_CALLER_B64)
                });
    }

    public static String missingField() throws Exception {
        return LoaderPhase0Fixtures.repeatedFailure(
                "phase0.MissingFieldCaller",
                new String[] { "phase0.MissingFieldCaller", "phase0.MissingFieldOwner" },
                new byte[][] {
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.MISSING_FIELD_CALLER_B64),
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.MISSING_FIELD_OWNER_NO_FIELD_B64)
                });
    }

    public static String missingMethod() throws Exception {
        return LoaderPhase0Fixtures.repeatedFailure(
                "phase0.MissingMethodCaller",
                new String[] { "phase0.MissingMethodCaller", "phase0.MissingMethodOwner" },
                new byte[][] {
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.MISSING_METHOD_CALLER_B64),
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.MISSING_METHOD_OWNER_NO_METHOD_B64)
                });
    }

    public static String missingInterfaceMethod() throws Exception {
        return LoaderPhase0Fixtures.repeatedFailure(
                "phase0.MissingInterfaceCaller",
                new String[] {
                    "phase0.MissingInterfaceCaller",
                    "phase0.MissingInterface",
                    "phase0.MissingInterfaceImpl"
                },
                new byte[][] {
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.MISSING_INTERFACE_CALLER_B64),
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.MISSING_INTERFACE_NO_METHOD_B64),
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.MISSING_INTERFACE_IMPL_B64)
                });
    }

}
