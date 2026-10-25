; Visit every element of an array.
; prefix: foreach
; context: body
Int ${2:i} = 0
While ${2:i} < ${1:array}.Length
	${3:Form} ${4:element} = ${1:array}[${2:i}]
	$0
	${2:i} += 1
EndWhile
