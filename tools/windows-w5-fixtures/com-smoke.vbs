Set d = CreateObject("Scripting.Dictionary")
d.Add "prime", "w5"
If d.Item("prime") <> "w5" Then WScript.Quit 7
WScript.Echo "PRIME_W5_COM_OK"
WScript.Quit 0
