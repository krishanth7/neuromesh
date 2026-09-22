-- NMP/1 Ping encoder with arbitrary precision validation.
import System.Environment (getArgs)
import System.Exit (die)
import Data.Char (digitToInt)
import Data.List (intercalate)
uint :: String -> Bool
uint [] = False
uint s@(first:rest) = length s <= 20 && all (`elem` ['0'..'9']) s && (null rest || first /= '0') && (length s < 20 || s <= "18446744073709551615")
bytes :: String -> [String]
bytes [] = []
bytes (a:b:rest) = show (16 * digitToInt a + digitToInt b) : bytes rest
bytes _ = []
main :: IO ()
main = do
 args <- getArgs
 case args of
  [node,mid,nonce] | length node == 64 && all (`elem` "0123456789abcdefABCDEF") node && uint mid && uint nonce && mid /= "0" ->
   putStrLn ("{\"version\":1,\"message_id\":" ++ mid ++ ",\"sender\":[" ++ intercalate "," (bytes node) ++ "],\"message\":{\"type\":\"Ping\",\"body\":{\"nonce\":" ++ nonce ++ "}}}")
  _ -> die "expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)"
