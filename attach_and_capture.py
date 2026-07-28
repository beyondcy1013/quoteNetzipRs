import frida, sys, time, os

js_code = """
(function() {
    console.log("[JS] Attaching to live process...");
    
    var hooked = false;

    function check() {
        if (hooked) return;
        var pattern = "55 8b ec 6a fe 68 f8 42 12 10"; // Ask function finger-print
        Process.enumerateRanges('r-x').forEach(function (range) {
            try {
                var ms = Memory.scanSync(range.base, range.size, pattern);
                ms.forEach(function (m) {
                    console.log("[JS] Found Ask at " + m.address);
                    Interceptor.attach(m.address, {
                        onEnter: function(args) {
                            var match = false;
                            var captured = [];
                            for(var i=0; i<6; i++) {
                                try {
                                    var arg = this.context.esp.add(4 + i*4).readPointer();
                                    var s = arg.readAnsiString();
                                    if (s && s.indexOf("168") !== -1) {
                                        match = true;
                                        captured.push("arg[" + i + "]: " + s);
                                    }
                                } catch(e) {}
                            }
                            if (match) {
                                console.log("\\n[!!!] ASK HIT WITH LOGIN CREDENTIALS:");
                                captured.forEach(function(l) { console.log("  " + l); });
                            }
                        }
                    });
                    hooked = true;
                });
            } catch (e) {}
        });
        if (!hooked) setTimeout(check, 1000);
    }
    setTimeout(check, 100);
})();
"""

PID = 65436 # Latest from tasklist
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', lambda msg, data: print(msg))
    script.load()
    print(f"[*] Attached to PID {PID}. Waiting for authentication event...")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
