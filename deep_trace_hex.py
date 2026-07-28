import frida, sys, time, os

js_code = """
(function() {
    var main = Process.enumerateModules()[0];
    var base = main.base;
    console.log("[JS] Main module base: " + base);

    function toHex(buf) {
        var uint8 = new Uint8Array(buf);
        var hex = "";
        for (var i = 0; i < uint8.length; i++) {
            var h = uint8[i].toString(16);
            if (h.length < 2) h = "0" + h;
            hex += h + " ";
        }
        return hex;
    }

    var p_send = Process.getModuleByName("ws2_32.dll").getExportByName("send");
    if (p_send) {
        Interceptor.attach(p_send, {
            onEnter: function(args) {
                try {
                    var len = args[2].toInt32();
                    if (len === 524 || len === 292 || len === 364) {
                        var ptr = args[1];
                        var data = ptr.readByteArray(Math.min(len, 64));
                        console.log("\\n[send] len=" + len + " ptr=" + ptr);
                        console.log("  Data Head: " + toHex(data));

                        // If 524, check the auth part at offset 244
                        if (len === 524) {
                            var auth_data = ptr.add(244).readByteArray(32);
                            console.log("  Auth Part (Cipher) @244: " + toHex(auth_data));
                        }

                        var bt = Thread.backtrace(this.context, 1).map(function(addr) {
                            try { return DebugSymbol.fromAddress(addr).toString(); } catch(e) { return addr.toString(); }
                        }).join("\\n");
                        console.log("  Backtrace:\\n" + bt);
                    }
                } catch(e) {}
            }
        });
        console.log("[JS] send hooked with hex output.");
    }
})();
"""

PID = 60112 
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', lambda msg, data: print(msg))
    script.load()
    print(f"[*] Attached to PID {PID}. Ready for hex traces.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
