pub mod action_list;
pub mod ax_element;
pub mod build_context;
pub mod builder;
pub mod capabilities;
pub mod element;
pub mod element_bounds;
pub mod resolve;
pub mod roles;
pub mod surfaces;

pub use ax_element::AXElement;
pub use build_context::TreeBuildContext;
pub use builder::{build_subtree, window_element_for};
pub use capabilities::{copy_action_names, is_attr_settable, same_element};
pub use element::{
    ABSOLUTE_MAX_DEPTH, copy_ax_array, copy_bool_attr, copy_element_attr, copy_i64_attr,
    copy_string_attr, copy_value_typed, count_children, element_for_pid, resolve_element_name,
};
pub use element_bounds::read_bounds;
pub use resolve::{find_element_recursive, resolve_element_impl};
pub use roles::{ax_role_to_str, is_interactive_role};
pub use surfaces::{
    alert_for_pid, focused_surface_for_pid, is_menu_open, list_surfaces_for_pid,
    menu_element_for_pid, menubar_for_pid, popover_for_pid, sheet_for_pid,
};
