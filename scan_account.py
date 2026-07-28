import frida, sys, time, os

def on_message(message, data):
    if message['type'] == 'send':
        print(f"[*] {message['payload']}")
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
            // ASCII "168"
            var ms = Memory.scanSync(range.base, range.size, "31 36 38");
            ms.forEach(function (m) {
                results.push({addr: m.address, type: "ASCII"});
            });
            // UTF16 "168"
            var ms_u16 = Memory.scanSync(range.base, range.size, "31 00 36 00 38 00");
            ms_u16.forEach(function (m) {
                results.push({addr: m.address, type: "UTF16"});
            });
        } catch (e) {}
    });
    
    console.log("Found " + results.length + " matches for '168'");
    results.forEach(function (res) {
        try {
            var context = hexDump(res.addr.sub(32), 128);
            console.log("Match at " + res.addr + " (" + res.type + "): " + context);
        } catch(e) {}
    });
}

setTimeout(scanForAccount, 500);
"""

PID = 60112 # Updated to current PID
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Attached to PID {PID} and scanning... Press Ctrl+C to stop.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
