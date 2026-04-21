/// Build the /help response text from the registered plugin commands.
/// `user_cmds` and `admin_cmds` are (name, description) pairs derived from plugins.
/// Admin commands are included only when `is_admin` is true.
pub fn build_help_text(
    user_cmds: &[(&str, &str)],
    admin_cmds: &[(&str, &str)],
    is_admin: bool,
) -> String {
    let mut text = String::from("**Available Commands**\n\n");
    for (name, desc) in user_cmds {
        text.push_str(&format!("`/{}` — {}\n", name, desc));
    }

    if is_admin && !admin_cmds.is_empty() {
        text.push_str("\n**Admin Commands** (registered on the admin guild only)\n\n");
        for (name, desc) in admin_cmds {
            text.push_str(&format!("`/{}` — {}\n", name, desc));
        }
    }

    text
}
