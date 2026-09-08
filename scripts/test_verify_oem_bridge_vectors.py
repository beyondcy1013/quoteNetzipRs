import unittest

from verify_oem_bridge_mode2 import comparison_case


class VectorEvidenceTests(unittest.TestCase):
    def test_equal_values_retain_inputs_and_bits(self):
        row = comparison_case({'mode': 0}, 7, 1.0,
                              {'packed_amount_i32': 7, 'oem_amount': 1.0})
        self.assertTrue(row['equal'])
        self.assertEqual(row['input'], {'mode': 0})
        self.assertEqual(row['native']['oem_amount_f32_le_hex'], '0000803f')

    def test_signed_zero_is_not_bitwise_equal(self):
        row = comparison_case({}, 0, 0.0,
                              {'packed_amount_i32': 0, 'oem_amount': -0.0})
        self.assertFalse(row['equal'])

    def test_packed_mismatch_is_not_hidden_by_same_float(self):
        row = comparison_case({}, 1, 1.0,
                              {'packed_amount_i32': 2, 'oem_amount': 1.0})
        self.assertFalse(row['equal'])


if __name__ == '__main__':
    unittest.main()
