import frida, sys, time, os

EXE_PATH = r"Z:\netzipapi-rust-demo\netzip_api_bin\NetzipAPI\StockC#\网际风.exe"

js_code = """
(function() {
    console.log("[JS] Monitoring module loads...");
    
    var hooked = false;

    function checkModules() {
        if (hooked) return;
        
        // Scan for the 6-byte prologue pattern + specific offsets
        // 55 8b ec 6a fe 68
        var pattern = "55 8b ec 6a fe 68";
        Process.enumerateRanges('r-x').forEach(function (range) {
            try {
                var ms = Memory.scanSync(range.base, range.size, pattern);
                ms.forEach(function (m) {
                    // Check if it's likely Stock.dat (contains Netzip or similar nearby or just suspicious)
                    // For now, hook all 6-byte prologues found in NEW or non-standard regions
                    if (m.address.compare(ptr("0x70000000")) < 0 && m.address.compare(ptr("0x400000")) > 0) {
                        try {
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
                                        console.log("\\n[!!!] ASK HIT at " + this.returnAddress);
                                        captured.forEach(function(l) { console.log("  " + l); });
                                        send({type: "found"}, this.context.esp.readByteArray(128));
                                    }
                                }
                            });
                            hooked = true;
                        } catch(e) {}
                    }
                });
            } catch (e) {}
        });
        
        if (!hooked) setTimeout(checkModules, 1000);
    }

    setTimeout(checkModules, 1000);
})();
"""

def on_message(message, data):
    if message['type'] == 'send' and message['payload']['type'] == 'found':
        print("[*] Captured Potential Login Args!")
        # We can dump more here

try:
    print(f"[*] Spawning {EXE_PATH}...")
    device = frida.get_local_device()
    pid = device.spawn([EXE_PATH])
    session = device.attach(pid)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    device.resume(pid)
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
