use walkdir::WalkDir;

fn main() {
    println!("cargo:rerun-if-changed=schemas");

    // rebuild schemas using flatc (must be installed) when schemas change

    let flatc_installed = std::process::Command::new("flatc")
        .arg("--version")
        .status()
        .is_ok();

    if !flatc_installed {
        eprintln!(
            "flatc is not installed, skipping schema compilation, this will cause issues if you have modified the schema"
        );
        return;
    }

    println!("compiling flatbuffer schemas...");

    // recursively compile all .fbs files in the schemas directory
    for file in WalkDir::new("schemas").into_iter().filter_map(|f| f.ok()) {
        if !file.file_type().is_file() || !file.file_name().to_string_lossy().ends_with(".fbs") {
            continue;
        }

        let path = file.path().to_str().unwrap();

        let output = std::process::Command::new("flatc")
            .args(["-o", "./generated", "--rust", path])
            .output()
            .unwrap();

        println!("flatc -o ./generated --rust {}", path);

        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            eprintln!("flatc failed to compile schema: {}", stderr);
        }
    }
}
