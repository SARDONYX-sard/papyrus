; Count from 0 to a limit, like a for loop.
; prefix: for
; context: body
Int ${1:i} = 0
While ${1:i} < ${2:count}
	$0
	${1:i} += 1
EndWhile
