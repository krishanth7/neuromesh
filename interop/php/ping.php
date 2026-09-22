<?php
// NMP/1 Ping encoder. Avoid converting unsigned values to PHP signed integers.
function uint_decimal($s) {
    return preg_match('/\A(?:0|[1-9][0-9]{0,19})\z/', $s) === 1 && (strlen($s)<20 || strcmp($s,'18446744073709551615')<=0);
}
$a=array_slice($argv,1);
if(count($a)!==3 || preg_match('/\A[0-9a-fA-F]{64}\z/',$a[0])!==1 || !uint_decimal($a[1]) || !uint_decimal($a[2]) || $a[1]==='0') {
    fwrite(STDERR,"expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)\n"); exit(1);
}
$sender=implode(',',array_values(unpack('C*',hex2bin($a[0]))));
echo '{"version":1,"message_id":'.$a[1].',"sender":['.$sender.'],"message":{"type":"Ping","body":{"nonce":'.$a[2]."}}}\n";
