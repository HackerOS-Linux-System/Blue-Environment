fn main() {
    // Kompiluj jeden plik który importuje wszystkie - to generuje jeden slint_generated.rs
    // z WSZYSTKIMI komponentami dostępnymi przez include_modules!()
    slint_build::compile("ui/all.slint").expect("ui/all.slint");
}
