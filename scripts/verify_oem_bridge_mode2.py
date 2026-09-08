"""Compare built Rust diagnostic with native scalar modes; retain every vector."""
import json
import hashlib
from pathlib import Path
import struct
import subprocess
import sys

from test_oem_bridge_scalars import native_mode1_amount, native_category9, SHA


def comparison_case(inputs, packed, amount, rust):
    native_bits = struct.pack('<f', amount).hex()
    rust_bits = struct.pack('<f', rust['oem_amount']).hex()
    return {'input': inputs,
            'native': {'packed_amount_i32': packed, 'oem_amount': amount,
                       'oem_amount_f32_le_hex': native_bits},
            'rust': dict(rust, oem_amount_f32_le_hex=rust_bits),
            'equal': packed == rust['packed_amount_i32'] and native_bits == rust_bits}


def main():
    binary = Path(sys.argv[1]).resolve(strict=True)
    source = Path(__file__).resolve().parents[1] / 'examples/official_5188_bridge_scalars.rs'
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    hashes_before = {'binary': digest(binary), 'source': digest(source),
                     'verifier': digest(Path(__file__)),
                     'native_test': digest(Path(__file__).with_name('test_oem_bridge_scalars.py'))}
    vectors = []
    for low in range(4):
        for high in range(8):
            bits = low | (high << 5)
            packed, amount, _ = native_mode1_amount(164124, 188, mode=2, close=873, scale_bits=bits)
            result = subprocess.run([str(binary), '3600', '188', '164124', '0', '0',
                                     '2', '873', str(bits)], check=True, capture_output=True, text=True)
            row = json.loads(result.stdout)
            vectors.append(comparison_case(
                dict(mode=2, timestamp=3600, volume=188, amount=164124,
                     delta=0, category=0, close=873, scale_bits=bits), packed, amount, row))
            if row['packed_amount_i32'] != packed or struct.pack('<f', row['oem_amount']) != struct.pack('<f', amount):
                raise AssertionError(f'mode2 mismatch at scale bits {bits}: {row}')
    cases = [(164124, 188, 0, 0), (12345, 0, 0, 0),
             (2**63-1, 1, 0, 0), (-(2**63), 1, 0, 0),
             (164124, 188, 50, 9), (12345, -150, -150, 9)]
    for amount, volume, delta, category in cases:
        adjusted, expected_delta, _ = (native_category9(volume, delta, [0]*10)
                                       if category == 9 else (volume, delta, None))
        packed, expected, _ = native_mode1_amount(amount, adjusted & 0xffffffff)
        result = subprocess.run([str(binary), '3600', str(volume), str(amount), str(delta),
                                 str(category), '1'], check=True, capture_output=True, text=True)
        row = json.loads(result.stdout)
        vector = comparison_case(dict(mode=1, timestamp=3600, amount=amount,
                                      volume=volume, delta=delta, category=category),
                                 packed, expected, row)
        vector['native']['adjusted_delta_i64'] = expected_delta
        vector['native']['adjusted_volume_i64'] = adjusted
        vector['equal'] &= row['adjusted_delta_i64'] == expected_delta
        vectors.append(vector)
        if (row['adjusted_delta_i64'] != expected_delta or row['packed_amount_i32'] != packed
                or struct.pack('<f', row['oem_amount']) != struct.pack('<f', expected)):
            raise AssertionError(f'mode1/category mismatch: {row}')
        if amount in (2**63-1, -(2**63)) and not row['invalid_conversion']:
            raise AssertionError('missing invalid conversion flag')
    if digest(binary) != hashes_before['binary'] or digest(source) != hashes_before['source']:
        raise AssertionError('binary/source changed during verification')
    direct_cases = [0, 1, 2**24+1, 2**31-1, 2**31, 2**32+65536,
                    2**45+65536, -1, -12345, -3784729286159, 2**63-1, -(2**63)]
    for amount in direct_cases:
        packed, expected, _ = native_mode1_amount(amount, 188, mode=0)
        result = subprocess.run([str(binary), '3600', '188', str(amount), '0', '0', '0'],
                                check=True, capture_output=True, text=True)
        row = json.loads(result.stdout)
        vectors.append(comparison_case(dict(mode=0, timestamp=3600, amount=amount,
                                            volume=188, delta=0, category=0),
                                       packed, expected, row))
        if row['packed_amount_i32'] != packed or struct.pack('<f', row['oem_amount']) != struct.pack('<f', expected):
            raise AssertionError(f'direct amount mismatch: {row}')
    if digest(binary) != hashes_before['binary'] or digest(source) != hashes_before['source']:
        raise AssertionError('binary/source changed during direct-mode verification')
    report = {'schema': 'oem-bridge-native-rust-vectors-v1',
              'vectors': vectors,
              'mode2_scale_cases': 32, 'mode1_category_cases': len(cases),
              'direct_amount_cases': len(direct_cases),
              'native_rust_equal': True, 'native_session_parity': False,
              'executable_sha256': SHA, 'sha256': hashes_before,
              'binary_source_build_binding': 'requires matching build log',
              'scope': 'conditional conversion; synthetic category and copied state'}
    if len(sys.argv) == 3:
        path = Path(sys.argv[2])
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('x') as output:
            json.dump(report, output, indent=2)
            output.write('\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
