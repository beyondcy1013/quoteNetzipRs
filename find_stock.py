import frida, sys

def on_message(message, data):
    if message['type'] == 'send':
        print(f"[*] {message['payload']}")
    else:
        print(message)

js_code = """
console.log("[JS] Process modules:");
Process.enumerateModules().forEach(function(m) {
    console.log("  " + m.name + " (" + m.base + ") -> " + m.path);
});

var main_m = Process.enumerateModules()[0];
console.log("[JS] Imports of " + main_m.name + ":");
try {
    main_m.enumerateImports().forEach(function(i) {
        if (i.name.toLowerCase().indexOf("ask") !== -1 || i.module.toLowerCase().indexOf("stock") !== -1) {
            console.log("    " + i.name + " from " + i.module + " at " + i.address);
        }
    });
} catch(e) { console.log("    Error enumerating imports: " + e); }

function scan(pattern, name) {
    var ranges = Process.enumerateRanges('r--');
    ranges.forEach(function(r) {
        try {
            Memory.scan(r.base, r.size, pattern, {
                onMatch: function(address, size) {
                    console.log("[*] Found " + name + " at " + address + " (Range: " + r.base + " size " + r.size + ")");
                },
                onComplete: function() {}
            });
        } catch(e) {}
    });
}
scan("53 74 6f 63 6b 2e 64 61 74", "Stock.dat");
scan("41 73 6b", "Ask");
"""

pid = 55428
device = frida.get_local_device()
session = device.attach(pid)
script = session.create_script(js_code)
script.on('message', on_message)
script.load()
print("[*] Scan started. Press Ctrl+C to stop.")
sys.stdin.read()
