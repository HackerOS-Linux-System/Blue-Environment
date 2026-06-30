mod messages;
mod socket;
mod handler;

pub use messages::CompositorMessage;
#[allow(unused_imports)]
pub use messages::ShellMessage;

pub use socket::{init_ipc, broadcast};
#[allow(unused_imports)]
pub use socket::{
    ipc_socket_path, Clients,
    broadcast_workspace_switch, broadcast_start_menu_toggle,
    broadcast_window_opened, broadcast_window_closed, broadcast_idle_changed,
};
#[allow(unused_imports)]
pub use handler::handle_shell_message;
