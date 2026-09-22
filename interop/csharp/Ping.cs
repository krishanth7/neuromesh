// NMP/1 Ping encoder. ASCII validation is independent of current culture.
using System;
using System.Text.RegularExpressions;
public static class Ping {
    static bool Uint(string s) { return Regex.IsMatch(s,@"\A(0|[1-9][0-9]{0,19})\z") && (s.Length<20 || String.CompareOrdinal(s,"18446744073709551615")<=0); }
    public static int Main(string[] a) {
        if(a.Length!=3 || !Regex.IsMatch(a[0],@"\A[0-9a-fA-F]{64}\z") || !Uint(a[1]) || !Uint(a[2]) || a[1]=="0") {
            Console.Error.WriteLine("expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)"); return 1;
        }
        string[] sender=new string[32];
        for(int i=0;i<32;i++) sender[i]=Convert.ToByte(a[0].Substring(i*2,2),16).ToString(System.Globalization.CultureInfo.InvariantCulture);
        Console.WriteLine("{\"version\":1,\"message_id\":"+a[1]+",\"sender\":["+String.Join(",",sender)+"],\"message\":{\"type\":\"Ping\",\"body\":{\"nonce\":"+a[2]+"}}}");
        return 0;
    }
}
