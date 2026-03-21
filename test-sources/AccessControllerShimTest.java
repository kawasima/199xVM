import java.io.IOException;
import java.security.AccessControlContext;
import java.security.AccessController;
import java.security.BasicPermission;
import java.security.Permission;
import java.security.PrivilegedAction;
import java.security.PrivilegedActionException;
import java.security.PrivilegedExceptionAction;

public class AccessControllerShimTest {
    private static final class NamedPermission extends BasicPermission {
        NamedPermission(String name) {
            super(name);
        }
    }

    public static String runActionOverloads() {
        String a = AccessController.doPrivileged((PrivilegedAction<String>) () -> "one");
        AccessControlContext context = new AccessControlContext(null);
        String b = AccessController.doPrivileged((PrivilegedAction<String>) () -> "two", context);
        Permission[] perms = new Permission[] { new NamedPermission("demo") };
        String c = AccessController.doPrivileged(
            (PrivilegedAction<String>) () -> "three",
            context,
            perms
        );
        boolean permissionMatch = perms[0].implies(new NamedPermission("demo"))
            && !perms[0].implies(new NamedPermission("other"))
            && "".equals(perms[0].getActions());

        return a + "|" + b + "|" + c + "|" + permissionMatch;
    }

    public static String throwWrappedException() throws PrivilegedActionException {
        return AccessController.doPrivileged((PrivilegedExceptionAction<String>) () -> {
            throw new IOException("boom");
        });
    }
}
