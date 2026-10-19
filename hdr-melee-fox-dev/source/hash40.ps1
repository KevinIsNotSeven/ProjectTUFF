param([string[]]$Paths)
$t = New-Object 'uint32[]' 256
for($i=0;$i -lt 256;$i++){ $c=[uint32]$i; for($k=0;$k -lt 8;$k++){ if($c -band 1){ $c = [uint32](([uint32]3988292384) -bxor ($c -shr 1)) } else { $c = [uint32]($c -shr 1) } }; $t[$i]=$c }
foreach($p in $Paths){
  $crc=[uint32]4294967295
  foreach($b in [Text.Encoding]::ASCII.GetBytes($p)){ $crc = [uint32]($t[($crc -bxor $b) -band 0xFF] -bxor ($crc -shr 8)) }
  $crc = [uint32]($crc -bxor [uint32]4294967295)
  "$p => $([uint64]$crc + ([uint64]$p.Length -shl 32))"
}
