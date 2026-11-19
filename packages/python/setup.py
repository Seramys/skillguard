import os

from setuptools import setup
from wheel.bdist_wheel import bdist_wheel as _bdist_wheel


class bdist_wheel(_bdist_wheel):
    def finalize_options(self):
        super().finalize_options()
        self.root_is_pure = False

    def get_tag(self):
        _, _, plat = super().get_tag()
        override = os.environ.get("SKILLGUARD_WHEEL_PLATFORM")
        if override:
            return "py3", "none", override
        # The Linux binary is built as a static musl executable in CI. A
        # manylinux tag keeps pip on glibc-based distros from rejecting it;
        # the binary itself has no glibc dependency.
        if plat.startswith("linux_"):
            arch = plat.split("_", 1)[1]
            if arch == "x86_64":
                plat = "manylinux_2_17_x86_64"
            elif arch == "aarch64":
                plat = "manylinux_2_17_aarch64"
        return "py3", "none", plat


setup(cmdclass={"bdist_wheel": bdist_wheel})
