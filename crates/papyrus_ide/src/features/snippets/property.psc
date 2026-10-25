; A property with Get and Set, backed by a variable.
; prefix: property
; context: item
${1:Int} ${2:_value}

${1:Int} Property ${3:Value}
	${1:Int} Function Get()
		Return ${2:_value}
	EndFunction
	Function Set(${1:Int} value)
		${2:_value} = value
	EndFunction
EndProperty
$0