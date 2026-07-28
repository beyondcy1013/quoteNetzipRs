import frida, sys, time, os

js_code = """
function scan() {
    // Longer pattern for Ask: 55 8b ec 6a fe 68 f8 42 12 10
    var pattern = "55 8b ec 6a fe 68 f8 42 12 10";
    var results = [];
    Process.enumerateRanges('r-x').forEach(function (range) {
        try {
            var ms = Memory.scanSync(range.base, range.size, pattern);
            ms.forEach(function (m) {
                results.push(m.address);
            });
        } catch (e) {}
    });
    
    console.log("Found " + results.length + " HIGH-CONFIDENCE candidates for Ask");
    results.forEach(function (addr) {
         Interceptor.attach(addr, {
             onEnter: function(args) {
                 // SILENT: Only log if "168" is present in arguments
                 var match = false;
                 var results = [];
                 for(var i=0; i<6; i++) { // Check first 6 args
                     try {
                         var arg = this.context.esp.add(4 + i*4).readPointer();
                         var s = arg.readAnsiString();
                         if (s && s.indexOf("168") !== -1) {
                             match = true;
                             results.push("arg[" + i + "]: " + s);
                         }
                     } catch(e) {}
                 }
                 
                 if (match) {
                     console.log("\\n[TARGET HIT!] Ask called with 168 at " + addr);
                     results.forEach(function(l) { console.log("  " + l); });
                     // Dump the stack or related buffers if possible
                 }
             }
         });
    });
}
setTimeout(scan, 200);
"""

PID = 60112 
try:
    session = frida.attach(PID)
    script = session.create_script(js_code)
    script.on('message', lambda msg, data: print(msg))
    script.load()
    print(f"[*] Silent monitoring active. Waiting for 168... (Ctrl+C to stop)")
    sys.stdin.read()
except Exception as e:
    print(f"Error: {e}")
