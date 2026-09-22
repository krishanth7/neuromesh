# NMP/1 Ping encoder. Never pass u64 tokens through R double conversion.
a <- commandArgs(trailingOnly=TRUE)
uint <- function(s) {
  if (!grepl("^(0|[1-9][0-9]{0,19})$", s, perl=TRUE) || grepl("[^0-9]", s)) return(FALSE)
  if (nchar(s) < 20) return(TRUE)
  digits <- utf8ToInt(s); limit <- utf8ToInt("18446744073709551615")
  different <- which(digits != limit)
  length(different) == 0 || digits[different[1]] < limit[different[1]]
}
if (length(a) != 3 || nchar(a[1],type="bytes") != 64 || grepl("[^0-9a-fA-F]",a[1]) || !uint(a[2]) || !uint(a[3]) || a[2] == "0") {
  cat("expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)\n",file=stderr()); quit(status=1)
}
starts <- seq(1,63,2)
sender <- paste(strtoi(substring(a[1], starts, starts+1),16L),collapse=",")
cat(paste0('{"version":1,"message_id":',a[2],',"sender":[',sender,'],"message":{"type":"Ping","body":{"nonce":',a[3],'}}}\n'))
