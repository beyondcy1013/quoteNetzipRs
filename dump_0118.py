import frida
import sys
import time
import os

OUTPUT_DIR = r'Z:\netzipapi-rust-demo'

def on_message(message, data):
    if message['type'] == 'send':
        msg = message['payload']
        name = msg.get('name')
        if name:
            full_path = os.path.join(OUTPUT_DIR, name)
            with open(full_path, 'wb') as f:
                f.write(data)
            print(f"[*] Dumped {len(data)} bytes to {full_path}")
        else:
            print(f"[*] Message: {msg}")
    else:
        print(f"[*] {message}")

js_code = """
var attached = false;

function tryAttach() {
    if (attached) return;
    try {
        var ws2 = Module.findExportByName("WS2_32.dll", "WSASend");
        var snd = Module.findExportByName("WS2_32.dll", "send");
        
        if (ws2) {
            console.log("[*] Hooking WSASend...");
            Interceptor.attach(ws2, {
                onEnter: function(args) {
                    var dwBufferCount = args[2].toInt32();
                    var lpBuffers = args[1];
                    for (var i = 0; i < dwBufferCount; i++) {
                        var len = lpBuffers.add(i * 8).readU32(); // len is first 4 bytes in WSABUF
                        var buf = lpBuffers.add(i * 8 + 4).readPointer();
                        if (len >= 280 && len <= 300) {
                            console.log("[WSASend] Possible 0x118! Length: " + len);
                            send({name: "cipher_0118_wsasend.bin"}, buf.readByteArray(len));
                        }
                    }
                }
            });
        }
        
        if (snd) {
            console.log("[*] Hooking send...");
            Interceptor.attach(snd, {
                onEnter: function(args) {
                    var len = args[2].toInt32();
                    var buf = args[1];
                    if (len >= 280 && len <= 300) {
                        console.log("[send] Possible 0x118! Length: " + len);
                        send({name: "cipher_0118_send.bin"}, buf.readByteArray(len));
                    }
                }
            });
        }
        
        // Also keep the internal hooks if they work
        var m = Process.findModuleByName("Stock.dat");
        if (m) {
            console.log("[*] Found Stock.dat, hooking internal 0x66b60...");
            var addr_plain = m.base.add(0x66b60);
            Interceptor.attach(addr_plain, {
                onEnter: function(args) { this.sav = [args[0], args[1], args[2], args[3]]; },
                onLeave: function(retval) {
                    for (var i=0; i<4; i++) {
                        try {
                            if (this.sav[i].toInt32() === 280) {
                                console.log("[Stock.dat] Found Plain at index " + i);
                                send({name: "plain_0118.bin"}, this.sav[i-1].readByteArray(280));
                            }
                        } catch(e) {}
                    }
                }
            });
        }

        attached = true;
    } catch(e) {
        // ...
    }
}

setInterval(tryAttach, 500);
"""

def main():
    pids = []
    if len(sys.argv) > 1:
        for arg in sys.argv[1:]:
            try: pids.append(int(arg))
            except: pass
    
    sessions = []
    device = frida.get_local_device()
    
    if pids:
        print(f"[*] Attaching to provided PIDs: {pids}")
        for pid in pids:
            try:
                session = device.attach(pid)
                print(f"[+] Attached to PID {pid}")
                sessions.append(session)
            except Exception as e:
                print(f"[!] Failed to attach to PID {pid}: {e}")
    else:
        keywords = ['网际风', 'tdxw', 'keyStock', '股票接收C#']
        print(f"[*] Enumerating processes to match keywords: {keywords}")
        processes = device.enumerate_processes()
        for p in processes:
            match = False
            try:
                # Try both as-is and as bytes to avoid encoding issues
                pname = p.name.lower()
                for kw in keywords:
                    if kw.lower() in pname:
                        match = True
                        break
            except: pass
            
            if match:
                try:
                    session = device.attach(p.pid)
                    print(f"[+] Attached to {p.name} (PID: {p.pid})")
                    sessions.append(session)
                except: pass

    if not sessions:
        print("[!] No target processes attached. Exiting.")
        sys.exit(1)

    for session in sessions:
        try:
            script = session.create_script(js_code)
            script.on('message', on_message)
            script.load()
        except Exception as e:
            print(f"[!] Failed to load script into session: {e}")
    
    print("[*] Monitoring 0x118 packets in attached processes...")
    
    # Keep running until files are found or user aborts
    timeout = 300 # 5 minutes
    elapsed = 0
    try:
        while elapsed < timeout:
            p_found = os.path.exists(os.path.join(OUTPUT_DIR, 'plain_0118.bin'))
            c_found = os.path.exists(os.path.join(OUTPUT_DIR, 'cipher_0118.bin'))
            if p_found and c_found:
                print(f"[+] SUCCESS: Captured both 0118 files in {OUTPUT_DIR}")
                for s in sessions:
                    try: s.detach()
                    except: pass
                sys.exit(0)
            time.sleep(1)
            elapsed += 1
        print("[-] Timeout reached (5 mins). Did not capture files.")
    except KeyboardInterrupt:
        pass
    
    for s in sessions:
        try: s.detach()
        except: pass

if __name__ == '__main__':
    main()
