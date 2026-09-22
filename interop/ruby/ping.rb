# NMP/1 Ping encoder with arbitrary precision integer validation.
require 'json'
def uint(s)
  /\A(?:0|[1-9][0-9]{0,19})\z/.match?(s) && s.to_i <= 18446744073709551615
end
a = ARGV
abort 'expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)' unless a.length == 3 && /\A[0-9a-fA-F]{64}\z/.match?(a[0]) && uint(a[1]) && uint(a[2]) && a[1] != '0'
puts JSON.generate(version: 1, message_id: a[1].to_i, sender: [a[0]].pack('H*').bytes, message: {type: 'Ping', body: {nonce: a[2].to_i}})
