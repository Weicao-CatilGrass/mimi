mod commands;
mod errors;
mod resources;

use commands::*;
use errors::*;
use resources::ResCurrentDir;

use mingling::prelude::*;
use mingling::setup::{BasicProgramSetup, GeneralRendererSetup};

fn main() {
    let mut program = ThisProgram::new();
    program.with_setup(BasicProgramSetup);
    program.with_setup(GeneralRendererSetup);

    // Inject current directory as a resource (captured once at startup)
    let cwd = std::env::current_dir().expect("cannot get current working directory");
    program.with_resource(ResCurrentDir(cwd));

    program.exec_and_exit();
}

gen_program!();
