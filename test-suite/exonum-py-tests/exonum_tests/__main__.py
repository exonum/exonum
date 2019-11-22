"""Runner of the Integration Test Suite"""
import unittest

# We want all the tests be available here, so it's okay.
# pylint: disable=wildcard-import, unused-wildcard-import
from exonum_tests import *

if __name__ == "__main__":
    # Run tests
    unittest.main()
