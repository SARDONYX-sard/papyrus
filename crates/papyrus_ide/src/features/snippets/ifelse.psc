; If with an Else block.
; prefix: ifelse
; context: body
If ${1:condition}
	$2
Else
	$0
EndIf
