fn main() {
    println!("cargo:rerun-if-changed=../../templates/default");
    println!("cargo:rerun-if-changed=../../templates/generators");
}
