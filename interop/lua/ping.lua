-- NMP/1 Ping encoder. Keep u64 values as validated decimal strings.
local function uint(s)
  return s ~= nil and s:match('^[0-9]+$') ~= nil and (#s == 1 or s:sub(1,1) ~= '0') and (#s < 20 or (#s == 20 and s <= '18446744073709551615'))
end
if #arg ~= 3 or #arg[1] ~= 64 or not arg[1]:match('^[0-9a-fA-F]+$') or not uint(arg[2]) or not uint(arg[3]) or arg[2] == '0' then
  io.stderr:write('expected: NODE_HEX MESSAGE_ID(1..u64) NONCE(0..u64)\n'); os.exit(1)
end
local sender={}
for i=1,64,2 do sender[#sender+1]=tostring(tonumber(arg[1]:sub(i,i+1),16)) end
io.write('{"version":1,"message_id":'..arg[2]..',"sender":['..table.concat(sender,',')..'],"message":{"type":"Ping","body":{"nonce":'..arg[3]..'}}}\n')
