; A repeated update: register, then handle OnUpdate.
; prefix: onupdate
; context: item
Event OnInit()
	RegisterForSingleUpdate(${1:1.0})
EndEvent

Event OnUpdate()
	$0
	RegisterForSingleUpdate(${1:1.0})
EndEvent
