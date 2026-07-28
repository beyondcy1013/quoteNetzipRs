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

function scanForPattern() {
    // Search for "18 01" at offset 14 within 280-byte blocks
    var results = [];
    Process.enumerateRanges('rw-').forEach(function (range) {
        try {
            var ms = Memory.scanSync(range.base, range.size, "18 01");
            ms.forEach(function (m) {
                // Check if it looks like the 0118 header nearby
                var head = m.address.sub(14);
                var lead = head.readU16();
                if (lead === 0 || lead === 0x180c) { // Check candidates
                    results.push(head);
                }
            });
        } catch (e) {}
    });
    
    console.log("Found " + results.length + " candidates for 0x118 body");
    results.forEach(function (addr) {
        var dump = hexDump(addr, 280);
        if (dump.indexOf("31 36 38") !== -1 || dump.indexOf("31 00 36 00 38 00") !== -1) {
            console.log("!!! TARGET PLAIN 0118 FOUND at " + addr);
            console.log(dump);
            send({type: "target", addr: addr}, addr.readByteArray(280));
        } else {
            // console.log("Candidate at " + addr + ": " + dump.substring(0, 64));
        }
    });
}

setTimeout(scanForPattern, 500);
"""

PID = 60112
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', on_message)
    script.load()
    print(f"[*] Attached to PID {PID} and scanning... Press Ctrl+C to stop.")
    
    def on_target(message, data):
        if message['type'] == 'send' and message['payload']['type'] == 'target':
            with open(r'Z:\netzipapi-rust-demo\plain_0118.bin', 'wb') as f:
                f.write(data)
            print(f"[*] Captured plain_0118.bin from {message['payload']['addr']}")

    script.on('message', on_target)
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
