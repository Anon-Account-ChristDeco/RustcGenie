use std::path::Path;

/// Returns `true` if the file should be skipped as a seed for splicing.
pub fn ignore_file_for_splicing(file: &Path) -> bool {
    let file_path_str = file.display().to_string();

    const LINE_LIMIT: usize = 400;

    let content = std::fs::read_to_string(file).unwrap_or_default();
    let lines_count = content.lines().count();

    lines_count > LINE_LIMIT
        || content.contains("no_core")
        || content.contains("lang_items")
        || content.contains("mir!(")
        || content.contains("break rust")
        || (content.contains("failure-status: 101") && content.contains("known-bug"))
        || EXCEPTIONS.iter().any(|e| file_path_str.contains(e))
        || file_path_str.contains("icemaker")
        || content.lines().any(|line| line.chars().nth(1000).is_some())
}

static EXCEPTIONS: &[&str] = &[
    // runtime
    "tests/ui/closures/issue-72408-nested-closures-exponential.rs",
    "tests/ui/issues/issue-74564-if-expr-stack-overflow.rs",
    "library/stdarch/crates/core_arch/src/mod.rs",
    // memory
    "tests/ui/issues/issue-50811.rs",
    "tests/ui/issues/issue-29466.rs",
    "src/tools/miri/tests/run-pass/float.rs",
    "tests/ui/numbers-arithmetic/saturating-float-casts-wasm.rs",
    "tests/ui/numbers-arithmetic/saturating-float-casts-impl.rs",
    "tests/ui/numbers-arithmetic/saturating-float-casts.rs",
    "tests/ui/wrapping-int-combinations.rs",
    // glacier/memory/time
    "fixed/23600.rs",
    "23600.rs",
    "fixed/71699.rs",
    "71699.rs",
    // runtime
    "library/stdarch/crates/core_arch/src/x86/avx512bw.rs",
    "library/stdarch/crates/core_arch/src/x86/mod.rs",
    "library/stdarch/crates/core_arch/src/lib.rs",
    // memory
    "tests/run-make-fulldeps/issue-47551/eh_frame-terminator.rs",
    // infinite recursion in rustdoc
    "tests/ui/recursion/issue-38591-non-regular-dropck-recursion.rs",
    "tests/ui/dropck/dropck_no_diverge_on_nonregular_2.rs",
    "tests/ui/dropck/dropck_no_diverge_on_nonregular_1.rs",
    // very slow
    "library/core/src/lib.rs",
    "library/stdarch/crates/core_arch/src/mod.rs",
    "compiler/rustc_middle/src/lib.rs",
    "library/stdarch/crates/core_arch/src/x86/avx512f.rs",
    "tests/ui/structs-enums/struct-rec/issue-84611.rs",
    "tests/ui/structs-enums/struct-rec/issue-74224.rs",
    "tests/ui/dropck/dropck_no_diverge_on_nonregular_3.rs",
    "library/portable-simd/crates/core_simd/src/lib.rs",
    "tests/ui-fulldeps/myriad-closures.rs",
    "src/tools/miri/tests/pass/float.rs",
    "library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs",
    "library/stdarch/crates/core_arch/src/aarch64/mod.rs",
    "library/stdarch/crates/core_arch/src/aarch64/neon/generated.rs",
    "library/stdarch/crates/core_arch/src/aarch64/neon/mod.rs",
    "src/tools/cargo/tests/testsuite/main.rs",
    "src/tools/clippy/clippy_lints/src/lib.rs",
    "library/stdarch/crates/stdarch-gen/src/main.rs",
    "src/tools/rust-analyzer/crates/proc-macro-srv/src/abis/abi_1_58/proc_macro/mod.rs",
    "src/tools/rust-analyzer/crates/proc-macro-srv/src/abis/abi_1_63/proc_macro/mod.rs",
    "tests/ui/issues/issue-22638.rs",
    "tests/ui/issues/issue-72933-match-stack-overflow.rs",
    "tests/ui/recursion/issue-86784.rs",
    "tests/ui/associated-types/issue-67684.rs",
];
