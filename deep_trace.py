import frida, sys, time, os

js_code = """
(function() {
    var main = Process.enumerateModules()[0];
    var base = main.base;
    console.log("[JS] Main module base: " + base);

    // 0x4694ed is the address in the backtrace. 
    // If base is 0x400000, then offset is 0x694ed.
    var p_wrapper = base.add(0x694ed); 
    console.log("[JS] Hooking wrapper at: " + p_wrapper);

    Interceptor.attach(p_wrapper, {
        onEnter: function(args) {
            try {
                // This function is likely called MANY times, so we filter.
                // We want to see if recent send calls relate to it.
                // For now, just log that it was hit.
            } catch(e) {}
        }
    });

    var p_send = Process.getModuleByName("ws2_32.dll").getExportByName("send");
    if (p_send) {
        Interceptor.attach(p_send, {
            onEnter: function(args) {
                try {
                    var len = args[2].toInt32();
                    if (len === 524 || len === 292 || len === 364) {
                        console.log("\\n[send] len=" + len + " ptr=" + args[1]);
                        var bt = Thread.backtrace(this.context, 1).map(function(addr) {
                            try { return DebugSymbol.fromAddress(addr).toString(); } catch(e) { return addr.toString(); }
                        }).join("\\n");
                        console.log("  Backtrace:\\n" + bt);
                        
                        // Dump registers of the CALLER (which is on the stack)
                        // The top of the stack usually contains the return address
                        // But we want to see the state of 网际风.exe just before it called send wrapper
                        
                        var ptr = args[1];
                        console.log("  Data Head: " + ptr.readByteArray(Math.min(len, 64)));
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
    print(f"[*] Attached to PID {PID}. Deep trace active.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
