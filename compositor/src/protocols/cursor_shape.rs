use smithay::delegate_cursor_shape;
use smithay::wayland::tablet_manager::TabletSeatHandler;
use crate::state::BlueState;

impl TabletSeatHandler for BlueState {}

delegate_cursor_shape!(BlueState);
