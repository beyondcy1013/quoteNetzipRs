import frida, sys, time, os

js_code = """
(function() {
    var main = Process.enumerateModules()[0];
    var base = main.base;
    console.log("[JS] Main module base: " + base);

    function getExp(mod, name) {
        try {
            var m = Process.getModuleByName(mod);
            return m.getExportByName(name);
        } catch(e) { return null; }
    }

    var p_send = getExp("ws2_32.dll", "send");
    if (p_send) {
        Interceptor.attach(p_send, {
            onEnter: function(args) {
                try {
                    var len = args[2].toInt32();
                    if (len === 524 || len === 292 || len === 364) {
                        console.log("\\n[send] Caught " + len + " byte packet.");
                        var bt = Thread.backtrace(this.context, 1).map(function(addr) {
                            try { return DebugSymbol.fromAddress(addr).toString(); } catch(e) { return addr.toString(); }
                        }).join("\\n");
                        console.log("  Backtrace:\\n" + bt);
                    }
                } catch(e) {}
            }
        });
        console.log("[JS] send hooked.");
    }
})();
"""

PID = 60112 
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', lambda msg, data: print(msg))
    script.load()
    print(f"[*] Attached to PID {PID}. Ready for traces.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
