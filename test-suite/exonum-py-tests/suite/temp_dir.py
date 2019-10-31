"""TempDir module."""
import tempfile
import shutil


class TempDir:
    """Class creating and managing the temporary directories."""

    def __init__(self, directory: str):
        self.temp_dir = directory

    @classmethod
    def create(cls) -> "TempDir":
        """Initializes a TempDir object."""
        temp_dir = tempfile.mkdtemp(prefix="exonum_test_suite_")

        return cls(temp_dir)

    def path(self) -> str:
        """Returns the path of the temporary dir."""
        return self.temp_dir

    def remove(self) -> None:
        """Removes created temporary directory."""
        shutil.rmtree(self.temp_dir)
