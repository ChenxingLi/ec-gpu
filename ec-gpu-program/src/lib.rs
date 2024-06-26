#[cfg(any(feature = "cuda", feature = "opencl"))]
mod program;
#[cfg(any(feature = "cuda", feature = "opencl"))]
pub use program::*;

#[cfg(not(any(feature = "cuda", feature = "opencl")))]
mod place_holder;

/// Errors of this library.
#[derive(thiserror::Error, Debug)]
pub enum EcError {
    /// A simple error that is described by a string.
    #[error("EcError: {0}")]
    Simple(&'static str),

    /// Error in case a GPU kernel execution was aborted.
    #[cfg(any(feature = "cuda", feature = "opencl"))]
    #[error("GPU call was aborted!")]
    Aborted,

    /// An error that is bubbled up from the rust-gpu-tools library.
    #[cfg(any(feature = "cuda", feature = "opencl"))]
    #[error("GPU tools error: {0}")]
    GpuTools(#[from] rust_gpu_tools::GPUError),

    /// IO error.
    #[error("Encountered an I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result wrapper that is always using [`EcError`] as error.
pub type EcResult<T> = std::result::Result<T, EcError>;
