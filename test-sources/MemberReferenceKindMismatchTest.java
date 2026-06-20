public class MemberReferenceKindMismatchTest {
    public static String run() throws Exception {
        String methodrefToInterface = LoaderPhase0Fixtures.repeatedFailure(
                "phase0.MethodrefToInterfaceCaller",
                new String[] { "phase0.MethodrefToInterfaceCaller", "phase0.InterfaceTarget" },
                new byte[][] {
                    LoaderPhase0Fixtures.bytesWithMethodReferenceTag(
                            LoaderPhase0Fixtures.METHODREF_TO_INTERFACE_CALLER_B64,
                            "phase0/InterfaceTarget",
                            "value",
                            10),
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.INTERFACE_TARGET_B64)
                });
        String interfaceMethodrefToClass = LoaderPhase0Fixtures.repeatedFailure(
                "phase0.InterfaceMethodrefToClassCaller",
                new String[] { "phase0.InterfaceMethodrefToClassCaller", "phase0.ClassTarget" },
                new byte[][] {
                    LoaderPhase0Fixtures.bytesWithMethodReferenceTag(
                            LoaderPhase0Fixtures.INTERFACE_METHODREF_TO_CLASS_CALLER_B64,
                            "phase0/ClassTarget",
                            "value",
                            11),
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.CLASS_TARGET_B64)
                });
        return new StringBuilder()
                .append(methodrefToInterface)
                .append('|')
                .append(interfaceMethodrefToClass)
                .toString();
    }

    public static String fieldAccessKindMismatch() throws Exception {
        String[] names = { "phase0.InstanceFieldCaller", "phase0.InstanceFieldOwner" };
        byte[] caller = LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.INSTANCE_FIELD_CALLER_B64);
        byte[] instanceAsStatic = LoaderPhase0Fixtures.bytesWithFieldStaticFlag(
                LoaderPhase0Fixtures.INSTANCE_FIELD_OWNER_B64,
                "instanceValue",
                true);
        byte[] staticAsInstance = LoaderPhase0Fixtures.bytesWithFieldStaticFlag(
                LoaderPhase0Fixtures.INSTANCE_FIELD_OWNER_B64,
                "staticValue",
                false);
        return new StringBuilder()
                .append(LoaderPhase0Fixtures.repeatedFailure(
                        "phase0.InstanceFieldCaller",
                        "getInstance",
                        names,
                        new byte[][] { caller, instanceAsStatic }))
                .append('|')
                .append(LoaderPhase0Fixtures.repeatedFailure(
                        "phase0.InstanceFieldCaller",
                        "putInstance",
                        names,
                        new byte[][] { caller, instanceAsStatic }))
                .append('|')
                .append(LoaderPhase0Fixtures.repeatedFailure(
                        "phase0.InstanceFieldCaller",
                        "getStatic",
                        names,
                        new byte[][] { caller, staticAsInstance }))
                .append('|')
                .append(LoaderPhase0Fixtures.repeatedFailure(
                        "phase0.InstanceFieldCaller",
                        "putStatic",
                        names,
                        new byte[][] { caller, staticAsInstance }))
                .toString();
    }

    public static String instanceFieldResolutionUsesCallerLoader() throws Exception {
        String[] names = { "phase0.InstanceFieldCaller", "phase0.InstanceFieldOwner" };
        byte[] caller = LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.INSTANCE_FIELD_CALLER_B64);
        String normal = LoaderPhase0Fixtures.repeatedFailure(
                "phase0.InstanceFieldCaller",
                "getInstance",
                names,
                new byte[][] {
                    caller,
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.INSTANCE_FIELD_OWNER_B64)
                });
        String loaderDistinct = LoaderPhase0Fixtures.repeatedFailure(
                "phase0.InstanceFieldCaller",
                "getInstance",
                names,
                new byte[][] {
                    caller,
                    LoaderPhase0Fixtures.bytesWithFieldStaticFlag(
                            LoaderPhase0Fixtures.INSTANCE_FIELD_OWNER_B64,
                            "instanceValue",
                            true)
                });
        return new StringBuilder().append(normal).append('|').append(loaderDistinct).toString();
    }

    public static String missingInstanceField() throws Exception {
        return LoaderPhase0Fixtures.repeatedFailure(
                "phase0.InstanceFieldCaller",
                "getInstance",
                new String[] { "phase0.InstanceFieldCaller", "phase0.InstanceFieldOwner" },
                new byte[][] {
                    LoaderPhase0Fixtures.bytes(LoaderPhase0Fixtures.INSTANCE_FIELD_CALLER_B64),
                    LoaderPhase0Fixtures.bytes(
                            LoaderPhase0Fixtures.INSTANCE_FIELD_OWNER_NO_FIELDS_B64)
                });
    }
}
