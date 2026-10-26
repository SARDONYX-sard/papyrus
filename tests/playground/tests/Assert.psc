ScriptName Assert Hidden
{Assertions of tests run by papyrus_analyzer test: a failure fails the test.}

Function IntEq(Int aiActual, Int aiExpected, String asMessage = "") Global Native
Function FloatEq(Float afActual, Float afExpected, Float afEpsilon = 0.0001, String asMessage = "") Global Native
Function StringEq(String asActual, String asExpected, String asMessage = "") Global Native
Function BoolEq(Bool abActual, Bool abExpected, String asMessage = "") Global Native
Function IsTrue(Bool abValue, String asMessage = "") Global Native
Function IsFalse(Bool abValue, String asMessage = "") Global Native
Function IsNone(Form akForm, String asMessage = "") Global Native
Function NotNone(Form akForm, String asMessage = "") Global Native
Function Fail(String asMessage) Global Native
