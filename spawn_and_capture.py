import frida, sys, time, os

# Target path
EXE_PATH = r"Z:\netzipapi-rust-demo\netzip_api_bin\NetzipAPI\StockC#\网际风.exe"

js_code = """
(function() {
    console.log("[JS] Script loaded in spawned process.");
    
    function scanAndHook() {
        var pattern = "55 8b ec 6a fe 68 f8 42 12 10"; // Ask function finger-print
        var ranges = Process.enumerateRanges('r-x');
        var found = 0;
        
        ranges.forEach(function (range) {
            try {
                var ms = Memory.scanSync(range.base, range.size, pattern);
                ms.forEach(function (m) {
                    found++;
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
                                // Dump 280 bytes if possible for auth body
                            }
                        }
                    });
                });
            } catch (e) {}
        });
        
        if (found === 0) {
            // If not found yet (maybe DLL not loaded), try again in 500ms
            setTimeout(scanAndHook, 500);
        } else {
            console.log("[JS] Ask hooked successfully.");
        }
    }

    // Start scanning
    setTimeout(scanAndHook, 1000);
})();
"""

def on_message(message, data):
    print(message)

try:
    print(f"[*] Spawning {EXE_PATH}...")
    device = frida.get_local_device()
    pid = device.spawn([EXE_PATH])
    session = device.attach(pid)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    
    print(f"[*] Script loaded. Resuming process {pid}...")
    device.resume(pid)
    
    print(f"[*] Waiting for 0x118 trigger. Press Ctrl+C to stop.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
