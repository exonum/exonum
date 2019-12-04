#!/usr/bin/env python
"""Setup Script for the Exonum Python Integration Tests."""
import setuptools

INSTALL_REQUIRES = ["exonum-launcher==0.1.3"]

PYTHON_REQUIRES = ">=3.6"

with open("README.md", "r") as readme:
    LONG_DESCRIPTION = readme.read()

setuptools.setup(
    name="exonum-integration-tests",
    version="0.1.0",
    author="The Exonum team",
    author_email="contact@exonum.com",
    description="Exonum Python Integration Tests",
    long_description=LONG_DESCRIPTION,
    long_description_content_type="text/markdown",
    url="https://github.com/exonum/exonum-python-client",
    packages=setuptools.find_packages(),
    install_requires=INSTALL_REQUIRES,
    python_requires=PYTHON_REQUIRES,
    classifiers=[
        "Programming Language :: Python :: 3",
        "License :: OSI Approved :: Apache Software License",
        "Operating System :: OS Independent",
        "Topic :: Security :: Cryptography",
    ],
)
