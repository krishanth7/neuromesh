// NMP/1 Ping encoder. Decimal strings preserve the full unsigned 64-bit range.
const args = process.argv.slice(2);
function uint(value: string): boolean {
  return /^(0|[1-9][0-9]{0,19})$/.test(value) && !/[^0-9]/.test(value) && BigInt(value) <= 18446744073709551615n;
}
if (args.length !== 3 || args[0].length !== 64 || /[^0-9a-fA-F]/.test(args[0]) || !args.slice(1).every(uint) || args[1] === '0') {
  console.error('expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)');
  process.exit(1);
}
const sender: number[] = Array.from({length: 32}, (_, i) => parseInt(args[0].slice(i*2, i*2+2), 16));
console.log(`{"version":1,"message_id":${args[1]},"sender":[${sender.join(',')}],"message":{"type":"Ping","body":{"nonce":${args[2]}}}}`);
