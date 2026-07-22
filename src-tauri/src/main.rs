// The shipped Windows .exe is a GUI app with NO console window (debug builds keep the
// console for logs). Never regress this — see the Definition of Done in
// `Build-Prompts-Guide.md`; verify the released binary, not a debug build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

fn main() {
    freally_player_lib::run();
}
