import frida, sys, time, os

def on_message(message, data):
    if message['type'] == 'send':
        print(message['payload'])
    else:
        print(message)

js_code = """
function hexDump(p, len) {
    var buf = p.readByteArray(len);
    var uint8 = new Uint8Array(buf);
    var hex = "";
    for (var i = 0; i < uint8.length; i++) {
        var h = uint8[i].toString(16);
        if (h.length < 2) h = "0" + h;
        hex += h + " ";
    }
    return hex;
}

function scanForAccount() {
    var results = [];
    Process.enumerateRanges('rw-').forEach(function (range) {
        try {
            var ms = Memory.scanSync(range.base, range.size, "31 36 38");
            ms.forEach(function (m) {
                results.push(m.address);
            });
        } catch (e) {}
    });
    
    console.log("Found " + results.length + " ASCII matches for '168'");
    
    // Check for pairs
    for (var i = 0; i < results.length; i++) {
        for (var j = i + 1; j < results.length; j++) {
            var diff = results[j].toInt32() - results[i].toInt32();
            if (diff > 0 && diff < 100) {
                console.log("HIGH CONFINCE PAIR at " + results[i] + " and " + results[j] + " (diff: " + diff + ")");
                var start = results[i].sub(32);
                console.log(hexDump(start, 280));
            }
        }
    }
}

setTimeout(scanForAccount, 500);
"""

PID = 60112
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Attached to PID {PID} and scanning... Press Ctrl+C to stop.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
