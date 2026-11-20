# SPDX-License-Identifier: Apache-2.0
"""Tests for scripts/check_wasm_parity.py: every test binary passes the same tests natively and on WASM.

Run with: python -m unittest discover -s scripts/tests
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import check_wasm_parity as parity  # noqa: E402

NATIVE = """
     Running unittests src/lib.rs (target/debug/build/sysml-model/1a2b/out/sysml_model-1a2b)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/corpus.rs (target/debug/build/sysml-corpus/3c4d/out/corpus-3c4d)
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running unittests src/lib.rs (target/debug/build/sysml-corpus/5e6f/out/sysml_corpus-5e6f)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests sysml_model
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
"""

WASM = """
     Running unittests src/lib.rs (target/wasm32-unknown-unknown/debug/build/sysml-model/7a8b/out/sysml_model-7a8b.wasm)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.01s
     Running tests/corpus.rs (target/wasm32-unknown-unknown/debug/build/sysml-corpus/9c0d/out/corpus-9c0d.wasm)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.02s
     Running unittests src/lib.rs (target/wasm32-unknown-unknown/debug/build/sysml-corpus/1e2f/out/sysml_corpus-1e2f.wasm)
no tests to run!
"""


class Parse(unittest.TestCase):
    def test_native_log(self):
        self.assertEqual(parity.parse(NATIVE), {
            ("sysml-model", "unittests src/lib.rs"): 4,
            ("sysml-corpus", "tests/corpus.rs"): 13,
            ("sysml-corpus", "unittests src/lib.rs"): 0,
        })

    def test_wasm_log_with_no_tests(self):
        self.assertEqual(parity.parse(WASM)[("sysml-corpus", "unittests src/lib.rs")], 0)

    def test_doc_tests_are_ignored(self):
        self.assertNotIn("sysml_model", str(parity.parse(NATIVE)))

    def test_a_failed_binary_counts_only_its_passed_tests(self):
        log = ("     Running tests/x.rs (target/debug/build/c/1/out/x-1)\n"
               "test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n")
        self.assertEqual(parity.parse(log), {("c", "tests/x.rs"): 2})


class Compare(unittest.TestCase):
    def test_equal_counts_pass(self):
        self.assertEqual(parity.compare(parity.parse(NATIVE), parity.parse(WASM)), [])

    def test_tests_that_do_not_run_on_wasm_fail(self):
        # The silent failure mode: plain #[test] is skipped by wasm-bindgen-test-runner.
        wasm = WASM.replace("test result: ok. 13 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.02s",
                            "no tests to run!")
        errors = parity.compare(parity.parse(NATIVE), parity.parse(wasm))
        self.assertEqual(len(errors), 1)
        self.assertIn("tests/corpus.rs", errors[0])
        self.assertIn("13 natively, 0 on WASM", errors[0])

    def test_a_binary_missing_on_wasm_fails(self):
        wasm = "\n".join(l for l in WASM.splitlines() if "sysml-model" not in l and "4 passed" not in l)
        errors = parity.compare(parity.parse(NATIVE), parity.parse(wasm))
        self.assertTrue(any("sysml-model" in e and "did not run on WASM" in e for e in errors), errors)

    def test_an_empty_log_fails(self):
        self.assertTrue(parity.compare({}, {}))


if __name__ == "__main__":
    unittest.main()
