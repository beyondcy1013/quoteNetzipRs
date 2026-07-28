import frida, sys, time, os

js_code = """
function scan() {
    // Ask pattern from file: 55 8b ec 6a fe 68
    var pattern = "55 8b ec 6a fe 68";
    var results = [];
    Process.enumerateRanges('r-x').forEach(function (range) {
        try {
            var ms = Memory.scanSync(range.base, range.size, pattern);
            ms.forEach(function (m) {
                results.push(m.address);
            });
        } catch (e) {}
    });
    
    console.log("Found " + results.length + " candidates for Stock.dat entry points");
    results.forEach(function (addr) {
         // Check if it's the right one by looking ahead
         var dump = addr.readByteArray(32);
         var u8 = new Uint8Array(dump);
         // Ask/Start/Stop often share the same prologue but have different stack/constant diffs
         console.log("Candidate at " + addr + ": " + addr.readByteArray(16));
         
         // Let's hook the candidate 
         Interceptor.attach(addr, {
             onEnter: function(args) {
                 console.log("\\n[Ask/Start/Stop Hit] at " + addr);
                 // Dump the first 8 arguments (standard EBP frame or simple stack)
                 for(var i=0; i<8; i++) {
                     try {
                         var arg = this.context.esp.add(4 + i*4).readPointer();
                         console.log("  arg[" + i + "]: " + arg);
                         // If it's a string pointer, dump it
                         try {
                              var s = arg.readAnsiString();
                              if (s && s.length > 0) console.log("    String: " + s);
                         } catch(e) {}
                     } catch(e) {}
                 }
             }
         });
    });
}
setTimeout(scan, 500);
"""

PID = 60112 
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', lambda msg, data: print(msg))
    script.load()
    print(f"[*] Monitoring PID {PID}... Press Ctrl+C to stop.")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
