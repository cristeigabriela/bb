#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Arch {
    X86,
    Amd64,
    Arm,
    Arm64,
}

impl Arch {
    #[must_use]
    pub const fn target_triple(self) -> &'static str {
        match self {
            Self::X86 => "i686-pc-windows-msvc",
            Self::Amd64 => "x86_64-pc-windows-msvc",
            Self::Arm => "thumbv7-pc-windows-msvc",
            Self::Arm64 => "aarch64-pc-windows-msvc",
        }
    }

    #[must_use]
    pub const fn defines(self) -> &'static [&'static str] {
        match self {
            Self::X86 => &["-D_WIN32", "-D_X86_", "-D_M_IX86=600"],
            Self::Amd64 => &[
                "-D_WIN32",
                "-D_WIN64",
                "-D_AMD64_",
                "-D_M_AMD64=100",
                "-D_M_X64=100",
            ],
            Self::Arm => &["-D_WIN32", "-D_ARM_", "-D_M_ARM=7"],
            Self::Arm64 => &["-D_WIN32", "-D_WIN64", "-D_ARM64_", "-D_M_ARM64=1"],
        }
    }
}
