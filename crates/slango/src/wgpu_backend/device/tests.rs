use super::first_name_containing;

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

#[test]
fn name_selection_finds_a_match_after_the_first_name() {
    let names = names(&[
        "llvmpipe (LLVM 19.1.7, 256 bits)",
        "NVIDIA GeForce RTX 4070",
    ]);
    assert_eq!(first_name_containing(&names, "NVIDIA"), Some(1));
}

#[test]
fn name_selection_takes_the_first_of_several_matches() {
    let names = names(&[
        "AMD Radeon",
        "NVIDIA GeForce RTX 4070",
        "NVIDIA GeForce RTX 3060",
    ]);
    assert_eq!(first_name_containing(&names, "NVIDIA"), Some(1));
}

#[test]
fn name_selection_matches_with_the_case_of_each_letter() {
    let names = names(&["NVIDIA GeForce RTX 4070"]);
    assert_eq!(first_name_containing(&names, "nvidia"), None);
}

#[test]
fn name_selection_without_a_match_finds_nothing() {
    let names = names(&[
        "llvmpipe (LLVM 19.1.7, 256 bits)",
        "NVIDIA GeForce RTX 4070",
    ]);
    assert_eq!(first_name_containing(&names, "Radeon"), None);
    assert_eq!(first_name_containing(&[], "Radeon"), None);
}
