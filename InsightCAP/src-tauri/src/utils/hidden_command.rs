use std::{ffi::OsStr, process::Command};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn std_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide_std_command_window(&mut command);
    command
}

pub fn tokio_command(program: impl AsRef<OsStr>) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    hide_tokio_command_window(&mut command);
    command
}

#[cfg(windows)]
fn hide_std_command_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_std_command_window(_command: &mut Command) {}

#[cfg(windows)]
fn hide_tokio_command_window(command: &mut tokio::process::Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_tokio_command_window(_command: &mut tokio::process::Command) {}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn uses_windows_create_no_window_flag() {
        assert_eq!(super::CREATE_NO_WINDOW, 0x08000000);
    }
}
