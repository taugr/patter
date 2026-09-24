# whisper-rs forwards CMAKE_* environment options to its vendored CMake project.
# Native CPU probing on virtual macOS runners can advertise unsupported i8mm.
# M1-compatible baseline; Metal acceleration remains enabled separately.
set(GGML_NATIVE OFF CACHE BOOL "Do not specialize for the build host" FORCE)
set(GGML_CPU_ARM_ARCH "armv8.2-a+fp16+dotprod" CACHE STRING "Apple Silicon baseline" FORCE)
