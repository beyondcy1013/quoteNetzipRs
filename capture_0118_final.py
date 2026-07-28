import frida, sys, time, os

def on_message(message, data):
    if message['type'] == 'send':
        print(message['payload'])
    else:
        print(message)

js_code = """
function hexDump(p, len) {
    try {
        var buf = p.readByteArray(len);
        var uint8 = new Uint8Array(buf);
        var hex = "";
        for (var i = 0; i < uint8.length; i++) {
            var h = uint8[i].toString(16);
            if (h.length < 2) h = "0" + h;
            hex += h + " ";
        }
        return hex;
    } catch(e) { return "read failed"; }
}

function scan() {
    var results = [];
    Process.enumerateRanges('rw-').forEach(function (range) {
        try {
            // Search for "18 01" (Cmd 0x118)
            var ms = Memory.scanSync(range.base, range.size, "18 01");
            ms.forEach(function (m) {
                // If 18 01 is at the expected header position in a 280-byte body
                var candidate = m.address.sub(14); 
                try {
                    var data = candidate.readByteArray(280);
                    var uint8 = new Uint8Array(data);
                    // Check if it looks like a valid header
                    if (uint8[2] === 0xfc) { // fc is length 252?
                         // Check for "168" (ASCII) in the body (starts at offset ~48?)
                         var s = "";
                         for(var i=0; i<280; i++) s += String.fromCharCode(uint8[i]);
                         if (s.indexOf("168") !== -1) {
                             console.log("!!! MATCH FOUND at " + candidate);
                             send({type: "found", addr: candidate}, data);
                         }
                    }
                } catch(e) {}
            });
        } catch (e) {}
    });
    console.log("Scan complete.");
}

setTimeout(scan, 500);
"""

PID = 60112
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    
    def on_found(message, data):
        if message['type'] == 'send' and message['payload']['type'] == 'found':
            with open(r'Z:\netzipapi-rust-demo\plain_0118.bin', 'wb') as f:
                f.write(data)
            print(f"[*] Captured plain_0118.bin from {message['payload']['addr']}")
            # Also read the cipher version from one of the bins
            # Actually we already have cipher_0118.bin from Step 1065
            print("[*] DONE. You have both plain_0118.bin and cipher_0118.bin!")

    script.on('message', on_found)
    script.load()
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
