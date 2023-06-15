
use std::*;
use gl_generator::*;

// ----------------------------------------------------------------------------------------------------

fn generate_gl_bindings() -> ()
{
    let directory = env::var("OUT_DIR").unwrap();
    let filepath = path::Path::new(&directory).join("gl_bindings.rs");
    let mut file = fs::File::create(&filepath).unwrap();
    Registry::new
    (
        Api::Gl,
        (4, 6),
        Profile::Core,
        Fallbacks::All,
        []
    ).write_bindings
    (
        StructGenerator, 
        &mut file
    ).unwrap()
}

// ----------------------------------------------------------------------------------------------------

fn main() -> ()
{
    println!("cargo:rerun-if-changed=build.rs");
    generate_gl_bindings();
    if cfg!(target_os = "windows")
    {
        winres::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .compile().unwrap();
    }
}
