/** NMP/1 Ping encoder; unsigned 64-bit tokens remain exact decimal strings. */
public final class Ping {
    static boolean uint(String s) {
        return s.matches("0|[1-9][0-9]{0,19}") && (s.length()<20 || s.compareTo("18446744073709551615")<=0);
    }
    public static void main(String[] a) {
        if (a.length!=3 || !a[0].matches("[0-9a-fA-F]{64}") || !uint(a[1]) || !uint(a[2]) || a[1].equals("0")) {
            System.err.println("expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)"); System.exit(1);
        }
        StringBuilder sender = new StringBuilder();
        for(int i=0;i<32;i++) { if(i>0) sender.append(','); sender.append(Integer.parseInt(a[0].substring(i*2,i*2+2),16)); }
        System.out.println("{\"version\":1,\"message_id\":"+a[1]+",\"sender\":["+sender+"],\"message\":{\"type\":\"Ping\",\"body\":{\"nonce\":"+a[2]+"}}}");
    }
}
