/* NMP/1 Ping encoder. Only validated tokens are emitted as JSON. */
#include <stdio.h>
#include <string.h>
static int uint64_decimal(const char *s) {
 size_t n=strlen(s);
 if(n==0 || n>20 || (n>1 && s[0]=='0')) return 0;
 for(size_t i=0;i<n;i++) if(s[i]<'0'||s[i]>'9') return 0;
 return n<20 || strcmp(s,"18446744073709551615")<=0;
}
static int nibble(char c) {
 if(c>='0'&&c<='9') return c-'0';
 if(c>='a'&&c<='f') return c-'a'+10;
 if(c>='A'&&c<='F') return c-'A'+10;
 return -1;
}
int main(int argc,char **argv) {
 if(argc!=4) goto invalid;
 if(strlen(argv[1])!=64 || !uint64_decimal(argv[2]) || !uint64_decimal(argv[3]) || strcmp(argv[2],"0")==0) goto invalid;
 unsigned sender[32];
 for(size_t i=0;i<32;i++) {
  int hi=nibble(argv[1][2*i]),lo=nibble(argv[1][2*i+1]);
  if(hi<0||lo<0) goto invalid;
  sender[i]=(unsigned)(hi*16+lo);
 }
 printf("{\"version\":1,\"message_id\":%s,\"sender\":[",argv[2]);
 for(size_t i=0;i<32;i++) printf("%s%u",i?",":"",sender[i]);
 printf("],\"message\":{\"type\":\"Ping\",\"body\":{\"nonce\":%s}}}\n",argv[3]);
 return ferror(stdout)?1:0;
invalid:
 fputs("expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)\n",stderr); return 1;
}
