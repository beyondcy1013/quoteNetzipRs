"""Conditional native instruction tests; not live callback parity."""
import hashlib
import os
from pathlib import Path
import struct
import unittest

import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_ECX, UC_X86_REG_EAX

EXE = Path(os.environ.get('NETZIP_NATIVE_EXE', 'samples/native/netzip.exe'))
SHA = 'de712a8dde6d990e1c586f8afd4194575e35dffa2d0f81245fe29f6f8509bd29'


def machine_for_record(volume):
    binary = EXE.read_bytes()
    if hashlib.sha256(binary).hexdigest() != SHA:
        raise ValueError('unexpected executable')
    pe = pefile.PE(data=binary)
    image = pe.get_memory_mapped_image()
    machine = Uc(UC_ARCH_X86, UC_MODE_32)
    base = pe.OPTIONAL_HEADER.ImageBase
    machine.mem_map(base, (len(image) + 4095) & ~4095)
    machine.mem_write(base, image)
    machine.mem_map(0x10000000, 0x10000)
    frame, source, dest = 0x10008000, 0x10001000, 0x10002000
    machine.reg_write(UC_X86_REG_EBP, frame)
    machine.reg_write(UC_X86_REG_ESP, frame - 0x100)
    machine.mem_write(frame + 8, struct.pack('<I', source))
    machine.mem_write(frame - 0x30, struct.pack('<I', dest))
    machine.mem_write(source + 0x14, struct.pack('<q', volume))
    return machine, source, dest


def execute(machine, start, stop):
    machine.emu_start(start, stop, count=10000)
    if machine.reg_read(UC_X86_REG_EIP) != stop:
        raise AssertionError('native slice did not reach its declared endpoint')


def native_scalars(timestamp, volume):
    machine, _, dest = machine_for_record(volume)
    machine.mem_write(dest + 0xc, struct.pack('<I', timestamp))
    execute(machine, 0x49af5e, 0x49af6a)
    execute(machine, 0x49b009, 0x49b04d)
    return (struct.unpack('<I', machine.mem_read(dest + 0xc, 4))[0],
            struct.unpack('<i', machine.mem_read(dest + 0x20, 4))[0])


def native_bridge_eligible(timestamp, reference, last, bid, ask):
    machine, source, _ = machine_for_record(0)
    for offset, value in [(0, timestamp), (0x12b, reference), (0x10, last),
                          (0x68, bid), (0x6c, ask)]:
        machine.mem_write(source + offset, struct.pack('<I', value & 0xffffffff))
    reached = []

    def stop_at_branch(machine, address, size, context):
        if address in (0x49ad87, 0x49adee):
            reached.append(address)
            machine.emu_stop()

    machine.hook_add(UC_HOOK_CODE, stop_at_branch)
    machine.emu_start(0x49ada7, 0x49adf8, count=100)
    if len(reached) != 1:
        raise AssertionError('eligibility branch did not terminate')
    return reached[0] == 0x49adee


def native_category9(volume, signed_delta, book):
    machine, source, _ = machine_for_record(volume)
    machine.mem_write(source + 0x24, struct.pack('<q', signed_delta))
    machine.mem_write(source + 0xa8, struct.pack('<10i', *book))
    # Enter after the resolved-symbol category == 9 branch, before any scaling.
    # The native 64-bit division helper executes directly from the mapped PE.
    execute(machine, 0x49ae3d, 0x49af01)
    return (struct.unpack('<q', machine.mem_read(source + 0x14, 8))[0],
            struct.unpack('<q', machine.mem_read(source + 0x24, 8))[0],
            struct.unpack('<10i', machine.mem_read(source + 0xa8, 40)))


def native_oem_volume(volume):
    machine, source, dest = machine_for_record(volume)
    # Model only the verified accepted-copy destination; no session binding.
    machine.mem_write(source + 0x206, struct.pack('<I', volume & 0xffffffff))
    machine.mem_write(0x10008000 + 8, struct.pack('<I', dest))
    machine.mem_write(0x10008000 + 12, struct.pack('<I', source))
    execute(machine, 0x4a7995, 0x4a79c6)
    return struct.unpack('<f', machine.mem_read(dest + 0x7c, 4))[0]


def native_bridge_amount(amount):
    machine, source, dest = machine_for_record(0)
    machine.mem_write(source + 0x1c, struct.pack('<q', amount))
    # Select the native scalar fallback; AVX-512 and live CPU selection are
    # outside this conditional test. Keep arithmetic instructions unmodified.
    flag = machine.mem_read(0x5e40c8, 1)[0]
    machine.mem_write(0x5e40c8, bytes([flag | 0x20]))
    execute(machine, 0x49af6a, 0x49af85)
    return struct.unpack('<f', machine.mem_read(dest + 0xe0, 4))[0]


def native_mode1_amount(amount, volume, mode=1, close=0, scale_bits=0):
    machine, source, dest = machine_for_record(0)
    frame = 0x10008000
    machine.mem_write(frame - 8, struct.pack('<I', dest))
    machine.mem_write(dest + 0x20, struct.pack('<I', volume))
    machine.mem_write(dest + 0xe0, struct.pack('<f', amount))
    machine.mem_write(dest + 0x18, struct.pack('<i', close))
    machine.mem_write(dest + 0xc4, bytes([scale_bits & 0xe3]))
    # Explicit compression branch, bypassing category selection only.
    if mode == 1:
        execute(machine, 0x40af68, 0x40afed)
    elif mode == 2:
        execute(machine, 0x40b01a, 0x40b107)
    elif mode == 0:
        machine.mem_write(0x5e40c8, bytes([machine.mem_read(0x5e40c8, 1)[0] | 0x20]))
        execute(machine, 0x40b109, 0x40b128)
    else:
        raise ValueError('unsupported test branch')
    packed = struct.unpack('<i', machine.mem_read(dest + 0x24, 4))[0]
    # Conditional accepted 256B copy, preserving the actual compressor output.
    machine.mem_write(source + 0x1e6, bytes(machine.mem_read(dest, 256)))
    machine.mem_write(frame + 8, struct.pack('<I', dest + 0x400))
    machine.mem_write(frame + 12, struct.pack('<I', source))
    execute(machine, 0x4a79c6, 0x4a79d7)
    result = struct.unpack('<f', machine.mem_read(dest + 0x480, 4))[0]
    constants = tuple(struct.unpack('<f', machine.mem_read(address, 4))[0]
                      for address in (0x5cdde0, 0x5cddd8, 0x5cdd60))
    return packed, result, constants


class NativeBridgeTests(unittest.TestCase):
    def test_accepted_copy_overwrites_zero_bytes(self):
        for alternate, last, minute_index in [(0, 873, 119), (901, 873, 65537),
                                              (0, 0, -1)]:
            with self.subTest(alternate=alternate, last=last, index=minute_index):
                machine, incoming, symbol = machine_for_record(0)
                frame = 0x10008000
                old = symbol + 0x1e6
                payload = bytearray(256)
                payload[0x18:0x1c] = struct.pack('<I', last)
                payload[0xb8:0xbc] = struct.pack('<I', alternate)
                machine.mem_write(incoming, bytes(payload))
                machine.mem_write(old - 1, b'\xcc' + b'\xa5' * 256 + b'\xdd')
                machine.mem_write(frame + 8, struct.pack('<I', symbol))
                machine.mem_write(frame + 0x10, struct.pack('<I', incoming))
                machine.mem_write(frame - 0x74, struct.pack('<I', old))
                machine.mem_write(frame - 0x7c, struct.pack('<i', minute_index))
                # All prior acceptance/side-effect processing is a precondition.
                execute(machine, 0x4a22f1, 0x4a2345)
                self.assertEqual(bytes(machine.mem_read(old, 256)), bytes(payload))
                self.assertEqual(bytes(machine.mem_read(old - 1, 1)), b'\xcc')
                self.assertEqual(bytes(machine.mem_read(old + 256, 1)), b'\xdd')
                self.assertEqual(struct.unpack('<I', machine.mem_read(symbol + 0x11e, 4))[0],
                                 alternate if alternate else last)
                self.assertEqual(struct.unpack('<H', machine.mem_read(symbol + 0x17b, 2))[0],
                                 minute_index & 0xffff)
                self.assertEqual(bytes(machine.mem_read(symbol + 0x181, 1)), b'\x01')

    def test_preopen_interval_predicate(self):
        for minute, allowance, expected in [(564, 5, 0), (565, 5, 1),
                                             (569, 5, 1), (570, 5, 0),
                                             (571, 5, 0), (569, 0, 0)]:
            with self.subTest(minute=minute, allowance=allowance):
                machine, symbol, _ = machine_for_record(0)
                frame = 0x10008000
                machine.mem_write(frame - 4, struct.pack('<I', symbol))
                machine.mem_write(frame - 8, struct.pack('<i', minute))
                machine.mem_write(symbol + 0xe6, bytes([9, 30]))
                machine.mem_write(symbol + 0xa7, struct.pack('<b', allowance))
                # Calendar conversion and category/zero-volume gates precede this slice.
                execute(machine, 0x40c483, 0x40c4ca)
                self.assertEqual(machine.reg_read(UC_X86_REG_EAX), expected)

    def test_schedule_interval_clamping(self):
        cases = [(570, 690, 568, -1), (570, 690, 569, 0),
                 (570, 690, 570, 0), (570, 690, 689, 119),
                 (570, 690, 690, 119), (570, 690, 710, 119),
                 (570, 690, 711, -1), (1320, 120, 1380, 60),
                 (1320, 120, 60, 180), (1320, 120, 121, 239),
                 (1320, 120, 141, -1)]
        for start, end, minute, expected in cases:
            with self.subTest(start=start, end=end, minute=minute):
                machine, symbol, _ = machine_for_record(0)
                frame = 0x10008000
                machine.mem_write(frame - 0x30, struct.pack('<I', symbol))
                machine.mem_write(frame - 0x2c, struct.pack('<i', minute))
                machine.mem_write(frame + 0xc, struct.pack('<i', -1))
                machine.mem_write(frame - 0x44, struct.pack('<i', 20))
                machine.mem_write(symbol + 0xe5, b'\x00')
                machine.mem_write(symbol + 0xea, struct.pack('<hhh', start, end, 120))
                # Start after calendar/category handling: one synthetic schedule.
                execute(machine, 0x40c2af, 0x40c426)
                actual = struct.unpack('<i', struct.pack('<I', machine.reg_read(UC_X86_REG_EAX)))[0]
                self.assertEqual(actual, expected)

    def test_time_transition_fast_path_bounds(self):
        for delta, fast in [(-201, False), (-200, True), (-199, True),
                            (0, True), (199, True), (200, False), (201, False)]:
            machine, current, old = machine_for_record(0)
            frame = 0x10008000
            machine.mem_write(frame + 0x10, struct.pack('<I', current))
            machine.mem_write(frame - 0x1500, struct.pack('<I', old))
            machine.mem_write(current + 0xc, struct.pack('<I', 1000 + delta))
            machine.mem_write(old + 0xc, struct.pack('<I', 1000))
            reached = []

            def stop(machine, address, size, context):
                if address in (0x4a2783, 0x4a278a):
                    reached.append(address)
                    machine.emu_stop()

            machine.hook_add(UC_HOOK_CODE, stop)
            machine.emu_start(0x4a2753, 0x4a278f, count=100)
            self.assertEqual(reached, [0x4a2783 if fast else 0x4a278a])

    def test_copy_volume_delta_gate_and_side_effect(self):
        for current_volume, old_volume, accepted in [
                (101, 100, True), (100, 100, True), (99, 100, False),
                (0, 0xffffffff, True), (0x80000000, 0, False),
                (0xffffffff, 0, False)]:
            with self.subTest(current=current_volume, old=old_volume):
                machine, current, old = machine_for_record(0)
                symbol = 0x10003000
                frame = 0x10008000
                machine.mem_write(frame + 0x10, struct.pack('<I', current))
                machine.mem_write(frame + 8, struct.pack('<I', symbol))
                machine.mem_write(frame - 0x74, struct.pack('<I', old))
                machine.mem_write(current + 0x20, struct.pack('<I', current_volume))
                machine.mem_write(old + 0x20, struct.pack('<I', old_volume))
                machine.mem_write(current + 0xe0, struct.pack('<f', 150.0))
                machine.mem_write(old + 0xe0, struct.pack('<f', 100.0))
                reached = []

                def stop(machine, address, size, context):
                    if address in (0x4a1e04, 0x4a1e28):
                        reached.append(address)
                        machine.emu_stop()

                machine.hook_add(UC_HOOK_CODE, stop)
                machine.emu_start(0x4a1dbb, 0x4a1e2c, count=100)
                self.assertEqual(reached, [0x4a1e28 if accepted else 0x4a1e04])
                # Even the negative-delta return follows this symbol-side write.
                self.assertEqual(struct.unpack('<f', machine.mem_read(symbol + 0x1b7, 4))[0], 50.0)
                self.assertEqual(struct.unpack('<I', machine.mem_read(old + 0x20, 4))[0], old_volume)

    def test_copy_change_predicate(self):
        cases = [({}, False), ({0xc: 101}, False), ({0xe0: 99}, False),
                 ({0x20: 101}, True), ({0xc: 99, 0x20: 101}, False),
                 ({0x18: 101}, True), ({0x88: 1}, True), ({0x48: 1}, True),
                 ({0x20: 99, 0x18: 101}, True), ({0x38: 1}, False)]
        for changes, expected in cases:
            with self.subTest(changes=changes):
                machine, current, old = machine_for_record(0)
                for record in (current, old):
                    for offset in (0xc, 0x20, 0x18):
                        machine.mem_write(record + offset, struct.pack('<I', 100))
                for offset, value in changes.items():
                    machine.mem_write(current + offset, struct.pack('<I', value))
                stack = machine.reg_read(UC_X86_REG_ESP)
                machine.mem_write(stack, struct.pack('<II', 0x10009000, old))
                machine.reg_write(UC_X86_REG_ECX, current)
                execute(machine, 0x40a9a0, 0x10009000)
                self.assertEqual(bool(machine.reg_read(UC_X86_REG_EAX)), expected)

    def test_bridge_entry_eligibility(self):
        cases = [
            ((0, 100, 100, 100, 100), False),
            ((1, 0, 100, 100, 100), False),
            ((1, 100, 0, 0, 0), False),
            ((1, 100, 100, 0, 0), True),
            ((1, 100, 0, 100, 0), True),
            ((1, 100, 0, 0, 100), True),
            ((1, -100, -100, 0, 0), True),
        ]
        for values, expected in cases:
            with self.subTest(values=values):
                self.assertEqual(native_bridge_eligible(*values), expected)

    def test_direct_amount_packing(self):
        for amount in [0, 1, 2**24+1, 2**31-1, 2**31, 2**32+65536, 2**45+65536]:
            with self.subTest(amount=amount):
                packed, result, _ = native_mode1_amount(amount, 188, mode=0)
                rounded = int(struct.unpack('<f', struct.pack('<f', amount))[0])
                expected_word = rounded if rounded <= 0x7fffffff else ((rounded >> 16) | 0x80000000) & 0xffffffff
                self.assertEqual(packed & 0xffffffff, expected_word)
                decoded = ((expected_word & 0x7fffffff) << 16) if expected_word & 0x80000000 else expected_word
                expected = struct.unpack('<f', struct.pack('<f', decoded))[0]
                self.assertEqual(result, expected)

    def test_direct_amount_signed_edge_controls(self):
        for source, packed, result in [
                (-1, -1, 140737488355328.0),
                (-12345, -1, 140737488355328.0),
                (-3784729286159, -57750384, 136952758140928.0),
                (2**63-1, -(2**31), 0.0)]:
            actual, output, _ = native_mode1_amount(source, 188, mode=0)
            self.assertEqual((actual, output), (packed, result))

    def test_category_selects_compression_and_scale_flags(self):
        for category, mode in [(0, 1), (15, 1), (86, 1),
                               (1, 2), (9, 2), (16, 2), (26, 2),
                               (3, 2), (8, 2), (18, 2), (23, 2)]:
            for unit, low in [(1, 0), (10, 1), (100, 2), (1000, 3), (7, 0)]:
                with self.subTest(category=category, unit=unit):
                    machine, metadata, state = machine_for_record(0)
                    machine.mem_write(metadata + 0x14, struct.pack('<I', category))
                    machine.mem_write(metadata + 0x9e, struct.pack('<H', unit))
                    machine.mem_write(metadata + 0x9c, bytes([2]))
                    machine.mem_write(state + 0x20, struct.pack('<I', 188))
                    machine.mem_write(state + 0x18, struct.pack('<i', 873))
                    machine.mem_write(state + 0xe0, struct.pack('<f', 164124))
                    stack = machine.reg_read(UC_X86_REG_ESP)
                    machine.mem_write(stack, struct.pack('<II', 0x10009000, metadata))
                    machine.reg_write(UC_X86_REG_ECX, state)
                    execute(machine, 0x40aeb0, 0x40b128)
                    flags = machine.mem_read(state + 0xc4, 1)[0]
                    self.assertEqual(flags, (2 << 5) | (mode << 2) | low)
                    packed = struct.unpack('<i', machine.mem_read(state + 0x24, 4))[0]
                    expected, _, _ = native_mode1_amount(
                        164124, 188, mode=mode, close=873, scale_bits=(2 << 5) | low)
                    self.assertEqual(packed, expected)

    def test_mode2_all_scale_bits(self):
        def f32(value):
            return struct.unpack('<f', struct.pack('<f', value))[0]

        for low in range(4):
            for high in range(8):
                with self.subTest(low=low, high=high):
                    low_scale = float(10**low)
                    high_scale = float(10**high if high <= 6 else 1)
                    amount, volume, close = 164124, 188, 873
                    packed, result, _ = native_mode1_amount(
                        amount, volume, mode=2, close=close, scale_bits=low | (high << 5))
                    unit = f32(f32(f32(f32(amount)/f32(volume))/low_scale)*high_scale)
                    raw = f32(f32(f32(unit-f32(close))*30.0)+0.5)
                    expected_packed = int(raw) if -(2**31) <= raw < 2**31 else -(2**31)
                    self.assertEqual(packed, expected_packed)
                    expected = f32(f32(f32(f32(packed)/30.0)+f32(close))*f32(volume))
                    expected = f32(f32(expected*low_scale)/high_scale)
                    self.assertEqual(result, expected)

    def test_mode1_invalid_conversion_continues(self):
        for amount in [2**63-1, -(2**63), float('nan'), float('inf')]:
            packed, result, _ = native_mode1_amount(amount, 1)
            self.assertEqual((packed, result), (-(2**31), -21473836.0))

    def test_mode2_compress_then_readback_unscaled(self):
        def f32(value):
            return struct.unpack('<f', struct.pack('<f', value))[0]

        for amount, volume, close in [(12345, 0, 873), (164124, 188, 873),
                                      (100000001, 12345, 8100), (-12345, 100, 42)]:
            packed, result, _ = native_mode1_amount(amount, volume, mode=2, close=close)
            if volume == 0:
                self.assertEqual((packed, result), (0, 0.0))
                continue
            per_unit = f32(f32(amount) / f32(volume))
            expected_packed = int(f32(f32(f32(per_unit-f32(close))*30.0)+0.5))
            self.assertEqual(packed, expected_packed)
            expected = f32(f32(f32(f32(packed)/30.0)+f32(close))*f32(volume))
            self.assertEqual(result, expected)

    def test_mode1_compress_then_oem_getter(self):
        def f32(value):
            return struct.unpack('<f', struct.pack('<f', value))[0]

        for amount, volume in [(0, 0), (12345, 0), (164124, 188),
                               (100000001, 12345), (-12345, 100)]:
            with self.subTest(amount=amount, volume=volume):
                packed, result, (offset, scale, rounding) = native_mode1_amount(amount, volume)
                if volume == 0:
                    self.assertEqual((packed, result), (0, 0.0))
                    continue
                expected_packed = int(f32(f32(f32(f32(amount)/f32(volume))-offset)*scale)+rounding)
                self.assertEqual(packed, expected_packed)
                expected = f32(f32(f32(f32(packed)/scale)+offset)*f32(volume))
                self.assertEqual(result, expected)

    def test_category9_wrapping(self):
        def signed(value, bits):
            return (value + 2**(bits-1)) % 2**bits - 2**(bits-1)

        def trunc100(value):
            return value // 100 if value >= 0 else -((-value) // 100)

        book = [2**31-1, -(2**31)] * 5
        expected_book = tuple(trunc100(signed(value+50, 32)) for value in book)
        for value in [2**63-1, -(2**63)]:
            expected_volume = trunc100(signed(value+50, 64))
            expected_delta = trunc100(signed(value+(50 if value > 0 else -50), 64))
            self.assertEqual(native_category9(value, value, book),
                             (expected_volume, expected_delta, expected_book))

    def test_signed_amount_float32(self):
        for value in [0, 1, -1, 2**24+1, -(2**24+1),
                      -3784729286159, 2**63-1, -(2**63)]:
            with self.subTest(amount=value):
                expected = struct.unpack('<f', struct.pack('<f', value))[0]
                self.assertEqual(native_bridge_amount(value), expected)

    def test_oem_volume_unsigned_float32(self):
        for value in [0, 17, 2**24+1, 2**31, -1, -520882088, 2**32+17]:
            with self.subTest(volume=value):
                expected = struct.unpack('<f', struct.pack('<f', value & 0xffffffff))[0]
                self.assertEqual(native_oem_volume(value), expected)

    def test_category9_rounding_and_negative_values(self):
        book = [-151, -150, -100, -51, -50, -1, 0, 1, 49, 50]
        expected_book = (-1, -1, 0, 0, 0, 0, 0, 0, 0, 1)
        for volume, delta, expected_volume, expected_delta in [
                (1, 1, 0, 0), (49, 49, 0, 0), (50, 50, 1, 1),
                (-150, -150, -1, -2), (0, 0, 0, 0)]:
            with self.subTest(volume=volume, delta=delta):
                self.assertEqual(native_category9(volume, delta, book),
                                 (expected_volume, expected_delta, expected_book))

    def test_volume_low_word(self):
        for source, expected in [(0, 0), (2**32 + 17, 17),
                                 (2**31, -2**31), (-520882088, -520882088)]:
            with self.subTest(volume=source):
                self.assertEqual(native_scalars(3600, source), (3600, expected))

    def test_utc_day_normalization(self):
        for source, expected in [(25199, 25199), (25200, 25200),
                                 (57600, 25200), (86399, 25200),
                                 (86400, 86400), (2**32-1, 2**32-1)]:
            with self.subTest(timestamp=source):
                self.assertEqual(native_scalars(source, 17), (expected, 17))


if __name__ == '__main__':
    unittest.main()
