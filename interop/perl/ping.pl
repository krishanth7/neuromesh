# NMP/1 Ping encoder; lexical comparison prevents unsigned precision loss.
use strict;
use warnings;
sub uint_decimal {
    my ($s)=@_;
    return $s =~ /\A(?:0|[1-9][0-9]{0,19})\z/ && (length($s)<20 || $s le '18446744073709551615');
}
my @a=@ARGV;
die "expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)\n" unless @a==3 && $a[0]=~/\A[0-9a-fA-F]{64}\z/ && uint_decimal($a[1]) && uint_decimal($a[2]) && $a[1] ne '0';
my $sender=join(',',unpack('C*',pack('H*',$a[0])));
print '{"version":1,"message_id":'.$a[1].',"sender":['.$sender.'],"message":{"type":"Ping","body":{"nonce":'.$a[2]."}}}\n";
