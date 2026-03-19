public class ProcessLauncherEchoMain {
    public static void main(String[] args) throws Exception {
        if (args.length > 0 && "bytes".equals(args[0])) {
            System.out.write((int) 'A');
            System.out.write(new byte[] { 'x', 'B', 'C', 'y' }, 1, 2);
            System.out.flush();
            System.err.write((int) 'D');
            System.err.write(new byte[] { 'x', 'E', 'F', 'y' }, 1, 2);
            System.err.flush();
            return;
        }

        if (args.length > 0 && "close".equals(args[0])) {
            System.out.write((int) 'A');
            System.out.close();
            System.err.write((int) 'B');
            System.err.close();
            return;
        }

        if (args.length > 0 && "write-after-close".equals(args[0])) {
            System.out.write((int) 'A');
            System.out.close();
            System.out.write((int) 'Z');
            System.out.flush();
            System.err.write((int) 'B');
            System.err.close();
            System.err.write((int) 'Y');
            System.err.flush();
            return;
        }

        if (args.length > 0 && "check-error-flush".equals(args[0])) {
            System.out.print("A");
            System.err.print("B");
            System.out.checkError();
            System.err.checkError();
            System.in.read();
            return;
        }

        System.out.print(args.length > 0 ? args[0] : "ready");
        System.out.print(">");

        int ch;
        while ((ch = System.in.read()) != -1) {
            if (ch == '!') {
                System.err.print("bang");
                return;
            }
            System.out.print(new String(new char[] { (char) ch }));
        }

        System.out.print("<eof>");
    }
}
