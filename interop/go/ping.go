// NMP/1 Ping encoder; validate all CLI inputs before constructing JSON.
package main
import ("encoding/hex"; "fmt"; "os"; "regexp"; "strconv"; "strings")
func uint(s string) bool {
 if !regexp.MustCompile(`^(0|[1-9][0-9]{0,19})$`).MatchString(s) { return false }
 _, err := strconv.ParseUint(s,10,64); return err == nil
}
func main() {
 a:=os.Args[1:]
 if len(a)!=3 || !regexp.MustCompile(`^[0-9a-fA-F]{64}$`).MatchString(a[0]) || !uint(a[1]) || !uint(a[2]) || a[1]=="0" { fmt.Fprintln(os.Stderr,"expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)"); os.Exit(1) }
 bytes,_:=hex.DecodeString(a[0]); sender:=make([]string,32)
 for i,b:=range bytes {sender[i]=strconv.Itoa(int(b))}
 fmt.Printf("{\"version\":1,\"message_id\":%s,\"sender\":[%s],\"message\":{\"type\":\"Ping\",\"body\":{\"nonce\":%s}}}\n",a[1],strings.Join(sender,","),a[2])
}
