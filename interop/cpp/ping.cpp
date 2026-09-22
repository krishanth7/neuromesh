// NMP/1 Ping encoder with lossless decimal validation.
#include <iostream>
#include <regex>
#include <string>
bool uint64_decimal(const std::string& s) {
 return std::regex_match(s,std::regex("0|[1-9][0-9]{0,19}")) && (s.size()<20 || s<="18446744073709551615");
}
int main(int argc,char**argv) {
 if(argc!=4 || !std::regex_match(argv[1],std::regex("[0-9a-fA-F]{64}")) || !uint64_decimal(argv[2]) || !uint64_decimal(argv[3]) || std::string(argv[2])=="0") {
  std::cerr<<"expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)\n"; return 1;
 }
 std::cout<<"{\"version\":1,\"message_id\":"<<argv[2]<<",\"sender\":[";
 for(int i=0;i<32;i++) std::cout<<(i?",":"")<<std::stoul(std::string(argv[1]+2*i,2),nullptr,16);
 std::cout<<"],\"message\":{\"type\":\"Ping\",\"body\":{\"nonce\":"<<argv[3]<<"}}}\n";
 return std::cout?0:1;
}
