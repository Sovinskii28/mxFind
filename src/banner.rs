use std::io::IsTerminal;

pub const BANNER_TEXT: &str = concat!(
    " __  __ __  __ _____ ___ _   _ ____  \n",
    "|  \\/  |\\ \\/ /|  ___|_ _| \\ | |  _ \\ \n",
    "| |\\/| | \\  / | |_   | ||  \\| | | | |\n",
    "| |  | | /  \\ |  _|  | || |\\  | |_| |\n",
    "|_|  |_|/_/\\_\\|_|   |___|_| \\_|____/ \n",
    "\n",
    "Matrix Federation Explorer\n",
    "version ",
    env!("CARGO_PKG_VERSION"),
);

pub fn print_banner() {
    if should_color() {
        println!("\x1b[36m{BANNER_TEXT}\x1b[0m");
    } else {
        println!("{BANNER_TEXT}");
    }
}

fn should_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}
