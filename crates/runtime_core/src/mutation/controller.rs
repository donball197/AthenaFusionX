use super::{MutationContract, PatchFunction};

pub struct MutationController;

impl MutationController {
    pub fn execute(patch: PatchFunction) -> MutationContract {
        println!("Executing surgical mutation: {}", patch.describe());

        MutationContract {
            target_changed: false,

            unrelated_files_unchanged: true,

            syntax_valid: false,

            tests_passed: false,

            memory_recorded: false,
        }
    }
}
